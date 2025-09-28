//! Trap handling functionality
//!
//! # Complete rCore Trap Handling Design & Process
//!
//! ## Architecture Overview
//! A **bidirectional trap handling system** that enables safe transitions between user and kernel space
//! through a unified entry/exit mechanism.
//!
//! ## Core Components
//!
//! ### 1. Hardware Integration
//! - **`stvec` CSR** → Points to `__alltraps` (set in [`init()`])
//! - **`sscratch` CSR** → Holds user stack pointer for fast stack switching
//! - **`scause`/`stval`** → Provide trap type and additional information
//!
//! ### 2. Assembly Layer (trap.S)
//! - **`__alltraps`** - Universal trap entry point
//! - **`__restore`** - Universal return-to-user mechanism
//! - **Stack switching** via `sscratch` register
//! - **Complete context preservation** (34 words: 32 regs + sstatus + sepc)
//!
//! ### 3. Rust Layer (this module)
//! - **[`trap_handler()`]** - Central dispatch logic
//! - **Trap classification** and appropriate response
//! - **Integration** with syscall and batch systems
//!
//! ## Complete Execution Flow
//!
//! ### User → Kernel Transition:
//! 1. **Hardware trap** (syscall/exception/interrupt)
//! 2. **`__alltraps`** entry
//! 3. **Stack switch**: `csrrw sp, sscratch, sp` (user→kernel)
//! 4. **Save context**: All registers → kernel stack as `TrapContext`
//! 5. **Call [`trap_handler()`]** with context pointer
//!
//! ### Kernel Processing:
//! 6. **Read `scause`** to determine trap type
//! 7. **Dispatch**:
//!    - **Syscalls** → [`syscall()`], increment `sepc += 4`
//!    - **Faults** → Kill app, `run_next_app()`
//!    - **Other** → Panic
//! 8. **Return modified context**
//!
//! ### Kernel → User Transition:
//! 9. **`__restore`** function
//! 10. **Restore context**: Load all registers from `TrapContext`
//! 11. **Restore CSRs**: `sstatus`, `sepc`, `sscratch`
//! 12. **Stack switch back**: `csrrw sp, sscratch, sp` (kernel→user)
//! 13. **`sret`**: Return to user space at `sepc` address
//!
//! ## Key Design Principles
//!
//! ### Unified Entry/Exit
//! - **Single `__alltraps`** for all trap types
//! - **Single `__restore`** for all returns to user space
//! - **Consistent context management** regardless of trap cause
//!
//! ### Stack Isolation
//! - **Separate user/kernel stacks** maintained via `sscratch`
//! - **Atomic stack switching** prevents security vulnerabilities
//! - **TrapContext on kernel stack** ensures kernel space safety
//!
//! ### Complete State Preservation
//! - **All 32 RISC-V registers** saved/restored
//! - **Critical CSRs** (`sstatus`, `sepc`) preserved
//! - **Exact program state restoration** after trap handling
//!
//! ### Batch System Integration
//! - **Failed apps killed immediately** (page faults, illegal instructions)
//! - **Next app launched automatically** via `run_next_app()`
//! - **System continues** even when individual apps fail
//!
//! ## Control Flow Summary
//! ```text
//! User App → [Trap] → __alltraps → save context → trap_handler →
//!                                                      ↓
//!                     dispatch (syscall/kill/panic) → modify context →
//!                                                      ↓
//!                     __restore → restore context → sret → User App
//! ```

mod context;

use crate::batch::run_next_app;
use crate::syscall::syscall;
use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Trap},
    stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read(); // get trap cause
    let stval = stval::read(); // get extra value
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // step over the ecall instruction, so that the user program can continue
            // from the next instruction after ecall
            cx.sepc += 4;
            // call the syscall dispatch function with syscall number and up to 3 arguments
            // syscall number is stored in a7 (x17) register
            // up to 3 arguments are stored in a0-a2 (x10-x12) registers
            // return value is stored in a0 (x10) register
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            // directly kill the current application in case of a page fault
            // and run the next application
            println!("[kernel] PageFault in application, kernel killed it.");
            run_next_app();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            // directly kill the current application in case of an illegal instruction
            // and run the next application
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            run_next_app();
        }
        _ => {
            // all other unsupported traps lead to a kernel panic
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub use context::TrapContext;
