# cython: language_level=3
"""Running statistics over typed memoryviews."""

import numpy as np
cimport numpy as cnp
cimport cython
from libc.math cimport sqrt, fabs
from libc.stdlib cimport malloc, free

ctypedef unsigned long long ullong
ctypedef double real

include "limits.pxi"

DEF BLOCK_SIZE = 64

ctypedef fused number:
    int
    double

cdef extern from "math.h" nogil:
    double hypot(double x, double y)
    double M_PI

cdef extern from "stdint.h":
    ctypedef int int32_t

cdef struct Point:
    double x
    double y

cpdef enum Mode:
    FAST = 0
    EXACT = 1

cdef int MAX_ITER = 0x40
cdef double TOLERANCE = 1.5e-9


cdef class RunningMean:
    """Mean of the values pushed so far."""

    cdef int count
    cdef double total
    cdef readonly str name
    cdef public list history

    def __cinit__(self, str name="mean"):
        self.count = 0
        self.total = 0.0
        self.name = name
        self.history = []

    cpdef void push(self, double value):
        self.count += 1
        self.total += value

    @property
    def mean(self):
        if self.count == 0:
            return None
        return self.total / self.count

    def __repr__(self):
        return f"RunningMean({self.name!r}, n={self.count:d})\n"


cdef double norm(double[:] a) nogil:
    cdef Py_ssize_t i
    cdef Py_ssize_t n = a.shape[0]
    cdef double acc = 0.0
    for i from 0 <= i < n:
        acc += a[i] * a[i]
    return sqrt(acc)


@cython.boundscheck(False)
@cython.wraparound(False)
def distances(cnp.ndarray[cnp.float64_t, ndim=2] points, int x, int y=0):
    """Distance from (x, y) to every row of `points`."""
    cdef int i, rows = points.shape[0]
    cdef double[:] out = np.zeros(rows, dtype=np.float64)
    cdef double[:, :] view = points
    with nogil:
        for i in range(rows):
            out[i] = hypot(view[i, 0] - x, view[i, 1] - y)
    return np.asarray(out)


cpdef ullong factorial(int n) except? 0:
    if n < 0 or n > 20:
        raise ValueError("n is out of range")
    cdef ullong result = 1
    while n > 1:
        result *= <ullong>n
        n -= 1
    return result


def scratch(int size):
    cdef Point* buffer = <Point*>malloc(size * sizeof(Point))
    if buffer is NULL:
        raise MemoryError()
    try:
        with nogil:
            buffer[0].x = fabs(-M_PI)
        return buffer[0].x, True, len(b'raw')
    finally:
        free(buffer)
