use crate::core::Hachimi;
use crate::il2cpp::symbols::{get_method_addr, Array};
use crate::il2cpp::types::Il2CppArray;
use crate::il2cpp::types::Il2CppImage;
use ureq;

type CompressRequestFn = extern "C" fn(data: *mut Il2CppArray) -> *mut Il2CppArray;
type DecompressResponseFn = extern "C" fn(data: *mut Il2CppArray) -> *mut Il2CppArray;

extern "C" fn CompressRequest(data: *mut Il2CppArray) -> *mut Il2CppArray {
    unsafe {
        let url = Hachimi::instance().config.load().notifier_host.clone() + "/notify/request";
        let buffer = Array::<u8>::from(data);
        let _ = ureq::Agent::new().post(&url).send_bytes(&buffer.as_slice());
    }
    get_orig_fn!(CompressRequest, CompressRequestFn)(data)
}
extern "C" fn DecompressResponse(data: *mut Il2CppArray) -> *mut Il2CppArray {
    let decompressed = get_orig_fn!(DecompressResponse, DecompressResponseFn)(data);
    unsafe {
        let url = Hachimi::instance().config.load().notifier_host.clone() + "/notify/response";
        let buffer = Array::<u8>::from(decompressed);
        let _ = ureq::Agent::new().post(&url).send_bytes(&buffer.as_slice());
    }
    decompressed
}

pub fn init(img: *const Il2CppImage) {
    get_class_or_return!(img, "Gallop", HttpHelper);

    let COMPRESSREQUEST_ADDR = get_method_addr(HttpHelper, c"CompressRequest", 1);
    let DECOMPRESSRESPONSE_ADDR = get_method_addr(HttpHelper, c"DecompressResponse", 1);

    new_hook!(COMPRESSREQUEST_ADDR, CompressRequest);
    new_hook!(DECOMPRESSRESPONSE_ADDR, DecompressResponse);
}
