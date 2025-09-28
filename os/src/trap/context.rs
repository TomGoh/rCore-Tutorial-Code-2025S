use riscv::register::sstatus::{self, Sstatus, SPP};
/// Trap Context
#[repr(C)]
pub struct TrapContext {
    /// general regs[0..31]
    pub x: [usize; 32],
    /// CSR sstatus      
    pub sstatus: Sstatus,
    /// CSR sepc
    pub sepc: usize,
}

impl TrapContext {
    /// set stack pointer to x_2 reg (sp)
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    
    /// Init app context with entry point and stack pointer,
    /// should be called before run an app, init an appropriate TrapContext for it
    ///
    /// The context setups include:
    /// 1. Set the entry point of the application to `sepc` field of TrapContext
    /// 2. Set the stack pointer of the application to `x[2]` (sp) field of TrapContext
    /// 3. Set the `sstatus` field by copying current kernel sstatus and setting
    ///    SPP (Supervisor Previous Privilege) to User mode, so that when `sret`
    ///    executes, the CPU will switch to user mode
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read(); // CSR sstatus
        // Switch the previous privilege mode from Supervisor to User
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry, // entry point of app
        };
        cx.set_sp(sp); // app's user stack pointer
        cx // return initial Trap Context of app
    }
}
