initSidebarItems({"fn":[["translate_opt_result","Translate Result<Option, E> into something C can understand, when T is not `#[repr(C)]`."],["translate_result","Translate Result<T, E>, into something C can understand, when T is not `#[repr(C)]`"],["translate_void_result","Identical to `translate_result`, but with additional type checking for the case that we have a `Result<(), E>` (which we're about to drop on the floor)."]],"struct":[["ExternError","Represents an error that occurred on the mentat side. Many mentat FFI functions take a `*mut ExternError` as the last argument. This is an out parameter that indicates an error that occurred during that function's execution (if any)."]]});