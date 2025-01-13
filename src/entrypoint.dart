// This file is the entrypoint for the Dart half of the application,
// because it is declared as such in `/runner/build.rs`.

import "dart:async";

import "package:tracing/zone.dart";

import "lib.dart" show run;

void main(List<String> args) =>
    runZoned(run, zoneSpecification: rustTracingZoneSpecification);
