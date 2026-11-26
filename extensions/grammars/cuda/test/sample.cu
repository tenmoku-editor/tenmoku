// SAXPY and a tiled reduction, with the launches that run them.
#include <cstdio>
#include <cuda_runtime.h>
#include "check.cuh"

#define BLOCK_SIZE 256

/* Scales and adds on the device. The block comment spans
   two lines. */
__device__ float axpy(float a, float x, float y) {
    return a * x + y;
}

__host__ __device__ inline int ceil_div(int n, int d) {
    return (n + d - 1) / d;
}

__constant__ float kBias = 0.5f;

__global__ void saxpy(int n, float a, const float *x, float *y) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        y[i] = axpy(a, x[i], y[i]) + kBias;
    }
}

template <typename T, int Tile>
__global__ void __launch_bounds__(BLOCK_SIZE) reduce(const T *in, T *out, int n) {
    __shared__ T tile[Tile];
    extern __shared__ char scratch[];
    unsigned int tid = threadIdx.x;
    unsigned int i = blockIdx.x * Tile + tid;
    tile[tid] = i < n ? in[i] : T(0);
    __syncthreads();
    for (unsigned int s = Tile / 2; s > 0; s >>= 1) {
        if (tid < s) {
            tile[tid] += tile[tid + s];
        }
        __syncthreads();
    }
    if (tid == 0) {
        out[blockIdx.x] = tile[0];
        printf("block %d: %f\n", blockIdx.x, static_cast<double>(tile[0]));
    }
}

__forceinline__ __device__ float square(float v) { return v * v; }

static const char *kUsage = R"(usage: saxpy [n]
  n defaults to 1 << 20)";

int main(int argc, char **argv) {
    const int n = argc > 1 ? atoi(argv[1]) : 1 << 20;
    const size_t bytes = n * sizeof(float);
    float *host_x = new float[n];
    float *host_y = new float[n];
    for (int i = 0; i < n; ++i) {
        host_x[i] = 1.0f;
        host_y[i] = 2.5e-1f;
    }

    float *x = nullptr;
    float *y = nullptr;
    cudaMalloc(&x, bytes);
    cudaMalloc(&y, bytes);
    cudaMemcpy(x, host_x, bytes, cudaMemcpyHostToDevice);
    cudaMemcpy(y, host_y, bytes, cudaMemcpyHostToDevice);

    dim3 block(BLOCK_SIZE);
    dim3 grid(ceil_div(n, BLOCK_SIZE));
    saxpy<<<grid, block>>>(n, 2.0f, x, y);

    cudaStream_t stream;
    cudaStreamCreate(&stream);
    float *partial = nullptr;
    cudaMalloc(&partial, grid.x * sizeof(float));
    reduce<float, BLOCK_SIZE><<<grid, block, BLOCK_SIZE * sizeof(float), stream>>>(y, partial, n);
    cudaStreamSynchronize(stream);

    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) {
        std::fprintf(stderr, "%s\n%s", cudaGetErrorString(err), kUsage);
        return 1;
    }

    cudaMemcpy(host_y, y, bytes, cudaMemcpyDeviceToHost);
    std::printf("y[0] = %.2f\n", host_y[0]);

    cudaStreamDestroy(stream);
    cudaFree(partial);
    cudaFree(x);
    cudaFree(y);
    delete[] host_x;
    delete[] host_y;
    return 0;
}
