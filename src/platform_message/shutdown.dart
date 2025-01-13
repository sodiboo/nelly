import "binary.dart";

Future<void> gracefulShutdown() async {
  final response =
      await sendPlatformMessage("nelly/graceful_shutdown", (writer) {});

  response.assertFinished();
}
