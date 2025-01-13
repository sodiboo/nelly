import "dart:ui";

import "binary.dart";

mixin XdgToplevelBinding {
  static Map<int, XdgToplevelBinding> instances = {};

  void close();
}

Future<int> create(
  String title,
  String appId,
  ViewConstraints? viewConstraints,
  XdgToplevelBinding binding,
) async {
  final response =
      await sendPlatformMessage("wayland/xdg_toplevel/create", (writer) {});

  final viewId = response.readInt64();
  response.assertFinished();

  XdgToplevelBinding.instances[viewId] = binding;

  await update(viewId, title, appId, viewConstraints);
  await initialCommit(viewId);
  return viewId;
}

Future<void> initialCommit(int viewId) async {
  final response = await sendPlatformMessage(
      "wayland/xdg_toplevel/initial_commit", (writer) {
    writer.writeInt64(viewId);
  });

  response.assertFinished();
}

Future<void> update(
  int viewId,
  String title,
  String appId,
  ViewConstraints? viewConstraints,
) async {
  await sendPlatformMessage("wayland/xdg_toplevel/update", (writer) {
    writer.writeInt64(viewId);
    writer.writeUtf8(title);
    writer.writeUtf8(appId);
  });

  if (viewConstraints != null) {
    await updateViewConstraints(viewId, viewConstraints);
  }
}

Future<void> updateViewConstraints(
  int viewId,
  ViewConstraints viewConstraints,
) async {
  await sendPlatformMessage("wayland/xdg_toplevel/update_view_constraints",
      (writer) {
    writer.writeInt64(viewId);
    writer.writeFloat64(viewConstraints.minWidth);
    writer.writeFloat64(viewConstraints.minHeight);
    writer.writeFloat64(viewConstraints.maxWidth);
    writer.writeFloat64(viewConstraints.maxHeight);
  });
}

Future<void> remove(int viewId) async {
  await sendPlatformMessage("wayland/xdg_toplevel/remove", (writer) {
    writer.writeInt64(viewId);
  });
  XdgToplevelBinding.instances.remove(viewId);
}

void initListeners() {
  registerPlatformMessageHandler("wayland/xdg_toplevel/close",
      (message, response) async {
    final viewId = message.readInt64();
    message.assertFinished();
    handleClose(viewId);
    // response is empty
  });
}

void handleClose(int viewId) {
  XdgToplevelBinding.instances[viewId]?.close();
}
