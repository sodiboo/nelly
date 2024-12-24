import "binary.dart";

Future<int> createXdgToplevel(String title, String appId) async {
  final response =
      await sendPlatformMessage("nelly/create_xdg_toplevel", (writer) {
    writer.writeUtf8(title);
    writer.writeUtf8(appId);
  });

  final viewId = response.readInt64();
  response.assertFinished();

  return viewId;
}

Future<void> updateXdgToplevel(int viewId, String title, String appId) async {
  await sendPlatformMessage("nelly/update_xdg_toplevel", (writer) {
    writer.writeInt64(viewId);
    writer.writeUtf8(title);
    writer.writeUtf8(appId);
  });
}

Future<void> removeXdgToplevel(int viewId) async {
  await sendPlatformMessage("nelly/remove_xdg_toplevel", (writer) {
    writer.writeInt64(viewId);
  });
}
