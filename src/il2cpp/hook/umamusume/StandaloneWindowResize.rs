use crate::il2cpp::api::il2cpp_resolve_icall;
use crate::il2cpp::symbols::get_method_addr;
use crate::il2cpp::types::Il2CppImage;
use crate::il2cpp::hook;

static mut GET_IS_VIRT: usize = 0;
impl_addr_wrapper_fn!(get_IsVirt, GET_IS_VIRT, bool,);

pub fn init(umamusume: *const Il2CppImage) {
    get_class_or_return!(umamusume, Gallop, StandaloneWindowResize);
    unsafe {
        GET_IS_VIRT = get_method_addr(StandaloneWindowResize,c"get_IsVirt",0);
    }
}
