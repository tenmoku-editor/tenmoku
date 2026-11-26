.text                        ; This file opens with a directive, not a comment:
.globl _start                ; tree-sitter-asm allows a newline only between
.align 4                     ; items, so a blank or comment first line parses
                             ; as an ERROR. Most real assembly opens that way,
                             ; which this sample therefore cannot do.

/*
 * Block comments are C style, and may span lines.
 */

_start:
    mov rax, 1               ; a decimal immediate
    mov rdi, 0x1f            ; hexadecimal
    mov rsi, 0b1010          ; binary
    mov rdx, -1              ; negative
    mov rcx, [rbx+8]         ; a memory operand with a displacement
    mov byte ptr [rbx], 0    ; a size keyword before a memory operand
    mov word ptr [rsi], 7
    mov dword ptr [rbp-4], 0
    mov qword ptr [rsp+8], rax
    add rsp, 0x20
    xor eax, eax
    ret

.L1:
    cmp eax, 42
    jne .L1
    jmp _start

# GAS users write their line comments with a hash, and this grammar takes
# those as well as the semicolon above.

section .data

msg:
    .ascii "Hello, world"
    .asciz 'single quotes too'

values:
    .byte 1, 2, 3, 4
    .word 0x1234, 0x5678
    .double 1.5, 2.5

// A third comment style: two slashes, which ARM's assembler accepts.

const MAX 8 * 8
const STRIDE 4 | 1

gas_x86:
    movq %rax, %rbx
    movl $42, %eax
    movl 8(%rbp), %ecx
    jmp *rel [0x10]

armv7:
    push {r4, lr}
    ldr r0, [r1, 4]
    mov r2, r3
    bl memcpy
    pop {r4, pc}
    bx lr

aarch64:
    stp x29, x30, [sp, 16]
    ldr x0, [x1]
    add x0, x0, x2
    ret
