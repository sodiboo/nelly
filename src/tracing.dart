import "dart:async";

import "package:stack_trace/stack_trace.dart";

import "ffi.dart" as nelly_ffi;
import "gen.dart";

const prefix = "$MANIFEST_DIR/";

void zonePrintToEmbedder(
    Zone self, ZoneDelegate parent, Zone zone, String line) {
  nelly_ffi.println(line);
}

String trimPrefix(String input) {
  if (input.startsWith(prefix)) {
    return input.substring(prefix.length);
  } else {
    return input;
  }
}

void log(int level, Trace trace, String message) {
  final caller = trace.frames[1];

  final file = trimPrefix(caller.uri.toFilePath());
  final line = caller.line ?? 0;
  final target = "nelly::dart/${caller.member ?? "<unknown>"}";

  nelly_ffi.log(
    level,
    target,
    file,
    line,
    message,
  );
}

/// The "error" level.
///
/// Designates very serious errors.
const logLevelError = 1;

/// The "warn" level.
///
/// Designates hazardous situations.
const logLevelWarn = 2;

/// The "info" level.
///
/// Designates useful information.
const logLevelInfo = 3;

/// The "debug" level.
///
/// Designates lower priority information.
const logLevelDebug = 4;

/// The "trace" level.
///
/// Designates very low priority, often extremely verbose, information.
const logLevelTrace = 5;

void error(String message) {
  log(logLevelError, Trace.current(), message);
}

void warn(String message) {
  log(logLevelWarn, Trace.current(), message);
}

void info(String message) {
  log(logLevelInfo, Trace.current(), message);
}

void debug(String message) {
  log(logLevelDebug, Trace.current(), message);
}

void trace(String message) {
  log(logLevelTrace, Trace.current(), message);
}
