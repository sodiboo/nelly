import "dart:async";
import "dart:convert";
import "dart:typed_data";
import "dart:ui";

import "package:flutter/services.dart";

import "../tracing.dart";

class BinaryWriter {
  final BytesBuilder builder;
  BinaryWriter([BytesBuilder? builder]) : builder = builder ?? BytesBuilder();

  void writeBytes(ByteBuffer bytes) {
    builder.add(Uint8List.view(bytes));
  }

  void writeUint8List(Uint8List list) {
    writeBytes(list.buffer);
  }

  void writeInt8List(Int8List list) {
    writeBytes(list.buffer);
  }

  void writeUint16List(Uint16List list) {
    writeBytes(list.buffer);
  }

  void writeInt16List(Int16List list) {
    writeBytes(list.buffer);
  }

  void writeUint32List(Uint32List list) {
    writeBytes(list.buffer);
  }

  void writeInt32List(Int32List list) {
    writeBytes(list.buffer);
  }

  void writeUint64List(Uint64List list) {
    writeBytes(list.buffer);
  }

  void writeInt64List(Int64List list) {
    writeBytes(list.buffer);
  }

  void writeUint8(int value) {
    writeUint8List(Uint8List.fromList([value]));
  }

  void writeInt8(int value) {
    writeInt8List(Int8List.fromList([value]));
  }

  void writeUint16(int value) {
    writeUint16List(Uint16List.fromList([value]));
  }

  void writeInt16(int value) {
    writeInt16List(Int16List.fromList([value]));
  }

  void writeUint32(int value) {
    writeUint32List(Uint32List.fromList([value]));
  }

  void writeInt32(int value) {
    writeInt32List(Int32List.fromList([value]));
  }

  void writeUint64(int value) {
    writeUint64List(Uint64List.fromList([value]));
  }

  void writeInt64(int value) {
    writeInt64List(Int64List.fromList([value]));
  }

  void writeUtf8(String str) {
    final strUtf8 = utf8.encode(str);
    writeUint64(strUtf8.length);
    writeUint8List(strUtf8);
  }
}

class BinaryReader {
  final ByteData data;
  int cursor = 0;
  BinaryReader(this.data);

  Uint8List readUint8List(int length) {
    final list = data.buffer.asUint8List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Int8List readInt8List(int length) {
    final list = data.buffer.asInt8List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Uint16List readUint16List(int length) {
    final list = data.buffer.asUint16List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Int16List readInt16List(int length) {
    final list = data.buffer.asInt16List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Uint32List readUint32List(int length) {
    final list = data.buffer.asUint32List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Int32List readInt32List(int length) {
    final list = data.buffer.asInt32List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Uint64List readUint64List(int length) {
    final list = data.buffer.asUint64List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  Int64List readInt64List(int length) {
    final list = data.buffer.asInt64List(cursor, length);
    cursor += length * list.elementSizeInBytes;
    return list;
  }

  int readUint8() {
    return readUint8List(1)[0];
  }

  int readInt8() {
    return readInt8List(1)[0];
  }

  int readUint16() {
    return readUint16List(1)[0];
  }

  int readInt16() {
    return readInt16List(1)[0];
  }

  int readUint32() {
    return readUint32List(1)[0];
  }

  int readInt32() {
    return readInt32List(1)[0];
  }

  int readUint64() {
    return readUint64List(1)[0];
  }

  int readInt64() {
    return readInt64List(1)[0];
  }

  String readUtf8() {
    final length = readUint64();
    final strUtf8 = readUint8List(length);
    return utf8.decode(strUtf8);
  }

  void assertFinished() {
    if (cursor != data.lengthInBytes) {
      throw Exception("Expected to have read ${data.lengthInBytes} bytes " +
          "but read $cursor bytes");
    }
  }
}

Future<ByteData> sendRawPlatformMessage(String channel, ByteData message) {
  final completer = Completer<ByteData>();

  info("sending platform message on channel $channel");

  PlatformDispatcher.instance.sendPlatformMessage(
    channel,
    message,
    (ByteData? response) {
      info("received platform message response on channel $channel");
      if (response == null) {
        completer.completeError(Exception("Received null response"));
      } else {
        completer.complete(response);
      }
    },
  );

  return completer.future;
}

Future<BinaryReader> sendPlatformMessage(
    String channel, FutureOr<void> Function(BinaryWriter) encode) async {
  final writer = BinaryWriter();
  await encode(writer);
  final message = writer.builder.takeBytes();

  info("sending platform message on channel $channel");
  final response =
      await sendRawPlatformMessage(channel, message.buffer.asByteData());
  info("received platform message response on channel $channel");

  // if (response == null) {
  //   throw Exception("Received null response");
  // }

  final reader = BinaryReader(response);

  return reader;
}
