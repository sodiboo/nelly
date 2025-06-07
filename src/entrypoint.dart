// This file is the entrypoint for the Dart half of the application,
// because it is declared as such in `/runner/build.rs`.

import "dart:async";

import "package:halcyon/tracing/setup.dart";

import "nelly.dart" as nelly;

void main(List<String> args) {
  runZoned(nelly.run, zoneSpecification: tracingZoneSpecification);
}
