// ffi symbols are snake case, but Dart will normally complain.
// ignore_for_file: non_constant_identifier_names

import "dart:convert";
import "dart:ffi";

// final class ByteSlice extends Struct {
//   external Pointer<Uint8> ptr;

//   @Size()
//   external int len;
// }

// final class OwnedString implements Finalizable {
//   final Pointer<Uint8> ptr;
//   final int len;

//   OwnedString(this.ptr, this.len);
// }

@Native<
    Void Function(
      Size level,
      Uint32 line,
      Pointer<Uint8> target,
      Size targetLength,
      Pointer<Uint8> message,
      Size messageLength,
      Pointer<Uint8> file,
      Size fileLength,
    )>(isLeaf: true)
external void nelly_ffi_log(
  int level,
  int line,
  Pointer<Uint8> target,
  int targetLength,
  Pointer<Uint8> message,
  int messageLength,
  Pointer<Uint8> file,
  int fileLength,
);

void log(int level, String target, String file, int line, String message) {
  final targetUtf8 = utf8.encode(target);
  final fileUtf8 = utf8.encode(file);
  final messageUtf8 = utf8.encode(message);

  nelly_ffi_log(
    level,
    line,
    targetUtf8.address,
    targetUtf8.length,
    fileUtf8.address,
    fileUtf8.length,
    messageUtf8.address,
    messageUtf8.length,
  );
}

@Native<Void Function(Pointer<Uint8> message, Size len)>(isLeaf: true)
external void nelly_ffi_println(Pointer<Uint8> message, int length);

void println(String msg) {
  final msgUtf8 = utf8.encode(msg);
  nelly_ffi_println(msgUtf8.address, msgUtf8.length);
}
