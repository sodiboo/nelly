import "dart:async";

import "package:generated/gen.dart" show WORKSPACE_DIR;
import "package:stack_trace/stack_trace.dart";
import "package:tracing/src/ffi.dart" as ffi;

const prefix = "$WORKSPACE_DIR/";

void zoneHandleUncaughtError(
  Zone self,
  ZoneDelegate parent,
  Zone zone,
  Object error,
  StackTrace stackTrace,
) {
  log(logLevelError, Trace.from(stackTrace), error.toString());
}

void zonePrint(
  Zone self,
  ZoneDelegate parent,
  Zone zone,
  String line,
) {
  ffi.println(line);
}

const rustTracingZoneSpecification = ZoneSpecification(
  handleUncaughtError: zoneHandleUncaughtError,
  print: zonePrint,
);

String trimPrefix(String input) {
  if (input.startsWith(prefix)) {
    return input.substring(prefix.length);
  } else {
    return input;
  }
}

void log(int level, Trace trace, String message) {
  final frame = trace.frames[0];
  final file = (frame.uri.scheme == "file")
      ? trimPrefix(frame.uri.toFilePath())
      : frame.uri.toString();
  final line = frame.line ?? 0;
  final target = "nelly::dart/${frame.member ?? "<unknown>"}";

  ffi.log(
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
  log(logLevelError, Trace.current(1), message);
}

void warn(String message) {
  log(logLevelWarn, Trace.current(1), message);
}

void info(String message) {
  log(logLevelInfo, Trace.current(1), message);
}

void debug(String message) {
  log(logLevelDebug, Trace.current(1), message);
}

void trace(String message) {
  log(logLevelTrace, Trace.current(1), message);
}
