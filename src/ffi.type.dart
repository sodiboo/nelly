import "dart:convert" show utf8;
import "dart:ffi" as ffi;

final class Str extends ffi.Struct {
  external ffi.Pointer<ffi.Uint8> data;

  @ffi.Size()
  external int len;

  @override
  String toString() {
    return utf8.decode(data.asTypedList(len));
  }
}
