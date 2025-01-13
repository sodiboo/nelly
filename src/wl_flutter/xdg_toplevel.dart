import "dart:ui";

import "package:flutter/rendering.dart";
import "package:flutter/widgets.dart";
import "package:tracing/tracing.dart";

import "../platform_message/xdg_toplevel.dart" as xdg_toplevel;
import "surface_lifecycle.dart";

class XdgToplevelSurface extends StatefulWidget {
  final String title;
  final String appId;

  final VoidCallback? onClose;

  final ViewConstraints? viewConstraints;

  final Widget child;

  const XdgToplevelSurface({
    super.key,
    required this.title,
    required this.appId,
    this.onClose,
    this.viewConstraints,
    required this.child,
  });

  @override
  State<XdgToplevelSurface> createState() => _XdgToplevelSurfaceState();
}

class _XdgToplevelSurfaceState extends State<XdgToplevelSurface>
    with
        WidgetsBindingObserver,
        SurfaceLifecycle,
        xdg_toplevel.XdgToplevelBinding {
  @override
  Future<int> createSurface() => xdg_toplevel.create(
        widget.title,
        widget.appId,
        widget.viewConstraints,
        this,
      );

  @override
  Future<void> updateSurface(int viewId, XdgToplevelSurface oldWidget) =>
      xdg_toplevel.update(
        viewId,
        widget.title,
        widget.appId,
        widget.viewConstraints,
      );

  @override
  Future<void> removeSurface(int viewId) => xdg_toplevel.remove(viewId);

  @override
  void close() {
    widget.onClose?.call();
  }

  @override
  Widget buildView(BuildContext context, FlutterView view) => View(
        view: view,
        child: (widget.viewConstraints != null)
            ? widget.child
            : ConstraintsTransformBox(
                alignment: Alignment.topLeft,
                constraintsTransform: (constraints) => constraints.loosen(),
                child: _XdgToplevelLayout(
                  view: view,
                  child: widget.child,
                ),
              ),
      );
}

class _XdgToplevelLayout extends SingleChildRenderObjectWidget {
  final FlutterView view;

  const _XdgToplevelLayout({
    required super.child,
    required this.view,
  });

  @override
  _XdgToplevelLayoutRenderBox createRenderObject(BuildContext context) =>
      _XdgToplevelLayoutRenderBox(view);

  @override
  void updateRenderObject(
    BuildContext context,
    covariant _XdgToplevelLayoutRenderBox renderObject,
  ) {
    renderObject.shouldCheckMaxSize = true;
    renderObject.view = view;
  }
}

class _XdgToplevelLayoutRenderBox extends RenderProxyBox {
  FlutterView view;
  bool shouldCheckMaxSize = true;

  _XdgToplevelLayoutRenderBox(this.view);

  ViewConstraints get viewConstraints => ViewConstraints(
        minWidth: minSize.width,
        minHeight: minSize.height,
        maxWidth: maxSize.width,
        maxHeight: maxSize.height,
      );

  Size _minSize = Size.zero;

  Size get minSize => _minSize;

  set minSize(Size value) {
    if (_minSize == value) return;

    _minSize = value;

    xdg_toplevel.updateViewConstraints(view.viewId, viewConstraints);
  }

  Size _maxSize = Size.infinite;

  Size get maxSize => _maxSize;

  set maxSize(Size value) {
    if (_maxSize == value) return;

    _maxSize = value;

    xdg_toplevel.updateViewConstraints(view.viewId, viewConstraints);
  }

  @override
  void performLayout() {
    super.performLayout();
    final RenderBox? child = this.child;
    if (child != null) {
//       debug("""
// child intrinsics:

// min width: ${child.getMinIntrinsicWidth(double.infinity)}
// max width: ${child.getMaxIntrinsicWidth(double.infinity)}
// min height: ${child.getMinIntrinsicHeight(double.infinity)}
// max height: ${child.getMaxIntrinsicHeight(double.infinity)}

// actual width: ${child.size.width}
// actual height: ${child.size.height}

// min width (zero): ${child.getMinIntrinsicWidth(0)}
// max width (zero): ${child.getMaxIntrinsicWidth(0)}
// min height (zero): ${child.getMinIntrinsicHeight(0)}
// max height (zero): ${child.getMaxIntrinsicHeight(0)}

// min width (100): ${child.getMinIntrinsicWidth(100)}
// max width (100): ${child.getMaxIntrinsicWidth(100)}
// min height (100): ${child.getMinIntrinsicHeight(100)}
// max height (100): ${child.getMaxIntrinsicHeight(100)}
//       """);
      final dryMinWidth = child.getMinIntrinsicWidth(double.infinity);
      final dryMinHeight = child.getMinIntrinsicHeight(double.infinity);

      final minWidth = child.getMaxIntrinsicWidth(dryMinHeight);
      final minHeight = child.getMaxIntrinsicHeight(dryMinWidth);

      minSize = Size(minWidth, minHeight);

      if (shouldCheckMaxSize) {
        shouldCheckMaxSize = false;
        child.layout(const BoxConstraints(), parentUsesSize: true);
        maxSize = child.size;
        debug("max size: $maxSize");
        super.performLayout();
        debug("child size: ${child.size}");
      }
    }
  }
}
