import "binary.dart";

enum Layer {
  background,
  bottom,
  top,
  overlay,
}

class Anchor {
  final bool top;
  final bool bottom;
  final bool left;
  final bool right;

  const Anchor({
    this.top = false,
    this.bottom = false,
    this.left = false,
    this.right = false,
  });
}

int serializeLayer(Layer layer) => switch (layer) {
      Layer.background => 0,
      Layer.bottom => 1,
      Layer.top => 2,
      Layer.overlay => 3,
    };

int serializeAnchor(Anchor anchor) =>
    (1 * (anchor.top as int)) |
    (2 * (anchor.bottom as int)) |
    (4 * (anchor.left as int)) |
    (8 * (anchor.right as int));

mixin WlrLayerBinding {
  static Map<int, WlrLayerBinding> instances = {};
}

Future<int> create(
  Layer layer,
  Anchor anchor,
  String namespace,
  WlrLayerBinding binding,
) async {
  final response =
      await sendPlatformMessage("wayland/wlr_layer/create", (writer) {
    writer.writeUint8(serializeLayer(layer));
    writer.writeUint8(serializeAnchor(anchor));
    writer.writeUtf8(namespace);
  });

  final viewId = response.readInt64();
  response.assertFinished();

  WlrLayerBinding.instances[viewId] = binding;

  // await initialCommit(viewId);
  return viewId;
}

Future<void> update(
  int viewId,
  int width,
  int height,
  Layer layer,
  Anchor anchor,
) async {
  await sendPlatformMessage("wayland/wlr_layer/update", (writer) {
    writer.writeInt64(viewId);
    writer.writeUint32(width);
    writer.writeUint32(height);
    writer.writeUint8(serializeLayer(layer));
    writer.writeUint8(serializeAnchor(anchor));
  });
}

Future<void> remove(int viewId) async {
  await sendPlatformMessage("wayland/wlr_layer/remove", (writer) {
    writer.writeInt64(viewId);
  });
  WlrLayerBinding.instances.remove(viewId);
}
