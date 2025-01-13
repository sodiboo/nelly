import "dart:ffi" as ffi;

final class SliceStr extends ffi.Struct {
  external ffi.Pointer<ffi.Uint8> data;

  @ffi.Size()
  external int len;
}
