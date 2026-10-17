	.build_version macos, 11, 0
	.section	__TEXT,__text,regular,pure_instructions
	.globl	_current
	.p2align	2
_current:
	.cfi_startproc
	lsr	x8, x2, #6
	cbz	x8, LBB0_3
	add	x9, x0, #96
	add	x10, x1, #32
LBB0_2:
	sub	x12, x9, #56
	ldur	d1, [x9, #-64]
	ldursb	w11, [x9, #-50]
	ldp	q5, q4, [x9, #-96]
	ld1.s	{ v1 }[2], [x12]
	dup.16b	v0, v5[1]
	mov.b	v0[1], v5[4]
	mov.b	v0[2], v5[7]
	sub	x12, x9, #8
	ldur	d3, [x9, #-16]
	ld1.s	{ v3 }[2], [x12]
	mov.b	v0[3], v5[10]
	ldp	q7, q6, [x9, #-48]
	dup.16b	v2, v7[1]
	mov.b	v0[4], v5[13]
	mov.b	v2[1], v7[4]
	mov.b	v2[2], v7[7]
	mov.b	v0[5], v4[0]
	mov.b	v2[3], v7[10]
	mov.b	v2[4], v7[13]
	mov.b	v0[6], v4[3]
	mov.b	v2[5], v6[0]
	mov.b	v2[6], v6[3]
	mov.b	v0[7], v4[6]
	mov.b	v2[7], v6[6]
	ldp	q7, q16, [x9]
	mov.b	v0[8], v4[9]
	dup.16b	v5, v7[1]
	mov.b	v5[1], v7[4]
	mov.b	v2[8], v6[9]
	mov.b	v5[2], v7[7]
	mov.b	v5[3], v7[10]
	mov.b	v0[9], v4[12]
	add	x12, x9, #40
	mov.b	v5[4], v7[13]
	ldr	d7, [x9, #32]
	mov.b	v2[9], v6[12]
	ld1.s	{ v7 }[2], [x12]
	mov.b	v5[5], v16[0]
	mov.b	v0[10], v4[15]
	mov.b	v5[6], v16[3]
	mov.b	v5[7], v16[6]
	mov.b	v2[10], v6[15]
	mov.b	v5[8], v16[9]
	mov.b	v5[9], v16[12]
	mov.b	v0[11], v1[2]
	ldursb	w12, [x9, #-2]
	mov.b	v5[10], v16[15]
	add	x13, x9, #88
	mov.b	v2[11], v3[2]
	ldp	q6, q4, [x9, #48]
	dup.16b	v16, v6[1]
	mov.b	v0[12], v1[5]
	mov.b	v16[1], v6[4]
	mov.b	v16[2], v6[7]
	mov.b	v2[12], v3[5]
	mov.b	v16[3], v6[10]
	mov.b	v16[4], v6[13]
	mov.b	v0[13], v1[8]
	ldr	d6, [x9, #80]
	ld1.s	{ v6 }[2], [x13]
	ldrsb	w13, [x9, #46]
	mov.b	v2[13], v3[8]
	mov.b	v5[11], v7[2]
	mov.b	v5[12], v7[5]
	mov.b	v0[14], v1[11]
	mov.b	v5[13], v7[8]
	mov.b	v16[5], v4[0]
	mov.b	v2[14], v3[11]
	mov.b	v16[6], v4[3]
	mov.b	v16[7], v4[6]
	mov.b	v5[14], v7[11]
	mov.b	v16[8], v4[9]
	mov.b	v16[9], v4[12]
	mov.b	v0[15], w11
	mov.b	v16[10], v4[15]
	mov.b	v16[11], v6[2]
	mov.b	v2[15], w12
	mov.b	v16[12], v6[5]
	mov.b	v16[13], v6[8]
	mov.b	v5[15], w13
	ldrsb	w11, [x9, #94]
	mov.b	v16[14], v6[11]
	mov.b	v16[15], w11
	stp	q0, q2, [x10, #-32]
	stp	q5, q16, [x10], #64
	add	x9, x9, #192
	subs	x8, x8, #1
	b.ne	LBB0_2
LBB0_3:
	ret
	.cfi_endproc

	.globl	_grouped
	.p2align	2
_grouped:
	.cfi_startproc
	lsr	x8, x2, #6
	cbz	x8, LBB1_3
	add	x9, x0, #96
LBB1_2:
	ldp	q5, q4, [x9, #-96]
	dup.16b	v0, v5[1]
	mov.b	v0[1], v5[4]
	mov.b	v0[2], v5[7]
	mov.b	v0[3], v5[10]
	sub	x10, x9, #56
	mov.b	v0[4], v5[13]
	mov.b	v0[5], v4[0]
	ldur	d5, [x9, #-64]
	mov.b	v0[6], v4[3]
	mov.b	v0[7], v4[6]
	ld1.s	{ v5 }[2], [x10]
	mov.b	v0[8], v4[9]
	mov.b	v0[9], v4[12]
	mov.b	v0[10], v4[15]
	mov.b	v0[11], v5[2]
	mov.b	v0[12], v5[5]
	ldursb	w10, [x9, #-50]
	mov.b	v0[13], v5[8]
	mov.b	v0[14], v5[11]
	mov.b	v0[15], w10
	ldp	q5, q4, [x9, #-48]
	dup.16b	v1, v5[1]
	mov.b	v1[1], v5[4]
	mov.b	v1[2], v5[7]
	mov.b	v1[3], v5[10]
	mov.b	v1[4], v5[13]
	sub	x10, x9, #8
	mov.b	v1[5], v4[0]
	mov.b	v1[6], v4[3]
	ldur	d5, [x9, #-16]
	mov.b	v1[7], v4[6]
	mov.b	v1[8], v4[9]
	ld1.s	{ v5 }[2], [x10]
	mov.b	v1[9], v4[12]
	mov.b	v1[10], v4[15]
	mov.b	v1[11], v5[2]
	mov.b	v1[12], v5[5]
	mov.b	v1[13], v5[8]
	ldursb	w10, [x9, #-2]
	mov.b	v1[14], v5[11]
	ldp	q5, q4, [x9]
	mov.b	v1[15], w10
	dup.16b	v2, v5[1]
	mov.b	v2[1], v5[4]
	mov.b	v2[2], v5[7]
	mov.b	v2[3], v5[10]
	mov.b	v2[4], v5[13]
	mov.b	v2[5], v4[0]
	add	x10, x9, #40
	mov.b	v2[6], v4[3]
	mov.b	v2[7], v4[6]
	ldr	d5, [x9, #32]
	mov.b	v2[8], v4[9]
	mov.b	v2[9], v4[12]
	ld1.s	{ v5 }[2], [x10]
	mov.b	v2[10], v4[15]
	mov.b	v2[11], v5[2]
	mov.b	v2[12], v5[5]
	mov.b	v2[13], v5[8]
	mov.b	v2[14], v5[11]
	ldrsb	w10, [x9, #46]
	mov.b	v2[15], w10
	ldp	q5, q4, [x9, #48]
	dup.16b	v3, v5[1]
	mov.b	v3[1], v5[4]
	mov.b	v3[2], v5[7]
	mov.b	v3[3], v5[10]
	add	x10, x9, #88
	mov.b	v3[4], v5[13]
	mov.b	v3[5], v4[0]
	ldr	d5, [x9, #80]
	mov.b	v3[6], v4[3]
	mov.b	v3[7], v4[6]
	ld1.s	{ v5 }[2], [x10]
	mov.b	v3[8], v4[9]
	mov.b	v3[9], v4[12]
	mov.b	v3[10], v4[15]
	mov.b	v3[11], v5[2]
	mov.b	v3[12], v5[5]
	ldrsb	w10, [x9, #94]
	mov.b	v3[13], v5[8]
	mov.b	v3[14], v5[11]
	mov.b	v3[15], w10
	st1.16b	{ v0, v1, v2, v3 }, [x1], #64
	add	x9, x9, #192
	subs	x8, x8, #1
	b.ne	LBB1_2
LBB1_3:
	ret
	.cfi_endproc

.subsections_via_symbols
