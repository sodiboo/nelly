import "package:macros/macros.dart";

macro class NellyFfi implements FunctionDefinitionMacro {
  const NellyFfi();

  @override
  void buildDefinitionForFunction(FunctionDeclaration function, FunctionDefinitionBuilder builder) {
    if (!function.hasExternal) {
      builder.report(Diagnostic(DiagnosticMessage(
          "Ffi bound functions must be marked external",
          target: function.asDiagnosticTarget,
        ), Severity.error));
    }

    if (function.hasBody) {
      return; // there is already a good builtin error if we don't emit one here
    }

    if (function.isGetter || function.isSetter || function.isOperator) {
      builder.report(Diagnostic(DiagnosticMessage(
          "Ffi bound functions must be plain functions",
          target: function.asDiagnosticTarget,
        ), Severity.error));
    }

    for (final param in function.typeParameters) {
      builder.report(Diagnostic(DiagnosticMessage(
          "Ffi bound functions may not take type parameters",
          target: param.asDiagnosticTarget,
        ), Severity.error));
    }

    for (final param in function.namedParameters) {
      builder.report(Diagnostic(DiagnosticMessage(
          "Ffi bound functions may not take named parameters",
          target: param.asDiagnosticTarget,
        ), Severity.error));
    }
  }
}
