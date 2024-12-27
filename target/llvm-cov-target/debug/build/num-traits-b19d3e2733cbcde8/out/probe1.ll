; ModuleID = 'probe1.f0ab053cbc0122ed-cgu.0'
source_filename = "probe1.f0ab053cbc0122ed-cgu.0"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

$__covrec_9BDF9AF0518BF006u = comdat any

$__profc__RNvCskF4mwvTcohZ_6probe15probe = comdat nodeduplicate

$__llvm_profile_filename = comdat any

@alloc_f93507f8ba4b5780b14b2c2584609be0 = private unnamed_addr constant <{ [8 x i8] }> <{ [8 x i8] c"\00\00\00\00\00\00\F0?" }>, align 8
@alloc_ef0a1f828f3393ef691f2705e817091c = private unnamed_addr constant <{ [8 x i8] }> <{ [8 x i8] c"\00\00\00\00\00\00\00@" }>, align 8
@__llvm_coverage_mapping = private constant { { i32, i32, i32, i32 }, [95 x i8] } { { i32, i32, i32, i32 } { i32 0, i32 95, i32 0, i32 6 }, [95 x i8] c"\02]\\x\DA\05\C1Q\0A\80 \0C\00\D0\BFn\E3\A6BE\10\DD\A2\03,3\DBG\1BM\83\BA}\EF\ADx\EA\95\F1f)/\93\22$\B2\A2h\B9pm\F6a\B5\84,{~!\19\B5\\\81\D5\0DG\18\F7\18\B7\8DB\EF}8P\9E\CB5#n\D5y\88\10\A6n&QY~\1AZ\1F\81" }, section "__llvm_covmap", align 8
@__covrec_9BDF9AF0518BF006u = linkonce_odr hidden constant <{ i64, i32, i64, i64, [9 x i8] }> <{ i64 -7214877721073291258, i32 9, i64 4092918765293829909, i64 7490770839605561212, [9 x i8] c"\01\01\00\01\01\01\01\002" }>, section "__llvm_covfun", comdat, align 8
@__profc__RNvCskF4mwvTcohZ_6probe15probe = private global [1 x i64] zeroinitializer, section "__llvm_prf_cnts", comdat, align 8
@__profd__RNvCskF4mwvTcohZ_6probe15probe = private global { i64, i64, i64, i64, ptr, ptr, i32, [3 x i16], i32 } { i64 -7214877721073291258, i64 4092918765293829909, i64 sub (i64 ptrtoint (ptr @__profc__RNvCskF4mwvTcohZ_6probe15probe to i64), i64 ptrtoint (ptr @__profd__RNvCskF4mwvTcohZ_6probe15probe to i64)), i64 0, ptr null, ptr null, i32 1, [3 x i16] zeroinitializer, i32 0 }, section "__llvm_prf_data", comdat($__profc__RNvCskF4mwvTcohZ_6probe15probe), align 8
@__llvm_prf_nm = private constant [37 x i8] c"\1F#x\DA\8B\0F\F2+s.\CEv3\C9-/\0BI\CE\CF\88\8A7+(\CAOJ54\05S\00\B6!\0B~", section "__llvm_prf_names", align 1
@llvm.compiler.used = appending global [1 x ptr] [ptr @__profd__RNvCskF4mwvTcohZ_6probe15probe], section "llvm.metadata"
@llvm.used = appending global [3 x ptr] [ptr @__llvm_coverage_mapping, ptr @__covrec_9BDF9AF0518BF006u, ptr @__llvm_prf_nm], section "llvm.metadata"
@__llvm_profile_filename = hidden constant [22 x i8] c"default_%m_%p.profraw\00", comdat

; <f64>::total_cmp
; Function Attrs: inlinehint nonlazybind uwtable
define internal i8 @_RNvMNtCsgwl3sW1YWUp_4core3f64d9total_cmpCskF4mwvTcohZ_6probe1(ptr align 8 %self, ptr align 8 %other) unnamed_addr #0 {
start:
  %right = alloca [8 x i8], align 8
  %left = alloca [8 x i8], align 8
  %self1 = load double, ptr %self, align 8
  %_4 = bitcast double %self1 to i64
  store i64 %_4, ptr %left, align 8
  %self2 = load double, ptr %other, align 8
  %_7 = bitcast double %self2 to i64
  store i64 %_7, ptr %right, align 8
  %_13 = load i64, ptr %left, align 8
  %_12 = ashr i64 %_13, 63
  %_10 = lshr i64 %_12, 1
  %0 = load i64, ptr %left, align 8
  %1 = xor i64 %0, %_10
  store i64 %1, ptr %left, align 8
  %_18 = load i64, ptr %right, align 8
  %_17 = ashr i64 %_18, 63
  %_15 = lshr i64 %_17, 1
  %2 = load i64, ptr %right, align 8
  %3 = xor i64 %2, %_15
  store i64 %3, ptr %right, align 8
  %_21 = load i64, ptr %left, align 8
  %_22 = load i64, ptr %right, align 8
  %4 = icmp sgt i64 %_21, %_22
  %5 = zext i1 %4 to i8
  %6 = icmp slt i64 %_21, %_22
  %7 = zext i1 %6 to i8
  %_0 = sub nsw i8 %5, %7
  ret i8 %_0
}

; probe1::probe
; Function Attrs: nonlazybind uwtable
define void @_RNvCskF4mwvTcohZ_6probe15probe() unnamed_addr #1 {
start:
  %0 = atomicrmw add ptr @__profc__RNvCskF4mwvTcohZ_6probe15probe, i64 1 monotonic, align 8
; call <f64>::total_cmp
  %_1 = call i8 @_RNvMNtCsgwl3sW1YWUp_4core3f64d9total_cmpCskF4mwvTcohZ_6probe1(ptr align 8 @alloc_f93507f8ba4b5780b14b2c2584609be0, ptr align 8 @alloc_ef0a1f828f3393ef691f2705e817091c)
  ret void
}

; Function Attrs: nounwind
declare void @llvm.instrprof.increment(ptr, i64, i32, i32) #2

attributes #0 = { inlinehint nonlazybind uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" }
attributes #1 = { nonlazybind uwtable "probe-stack"="inline-asm" "target-cpu"="x86-64" }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 2, !"RtLibUseGOT", i32 1}
!2 = !{!"rustc version 1.83.0 (90b35a623 2024-11-26)"}
