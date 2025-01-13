import "dart:ui";

import "package:flutter/widgets.dart";

import "../platform_message/wlr_layer.dart" as wlr_layer;
import "../platform_message/wlr_layer.dart" show Anchor, Layer;
import "surface_lifecycle.dart";
export "../platform_message/wlr_layer.dart" show Anchor, Layer;

class WlrLayerSurface extends StatefulWidget {
  final Layer layer;
  final Anchor anchor;
  final String namespace;
  final Widget child;

  const WlrLayerSurface({
    super.key,
    required this.layer,
    this.anchor = const Anchor(),
    required this.namespace,
    required this.child,
  });

  @override
  State<WlrLayerSurface> createState() => _WlrLayerSurfaceState();
}

class _WlrLayerSurfaceState extends State<WlrLayerSurface>
    with WidgetsBindingObserver, SurfaceLifecycle, wlr_layer.WlrLayerBinding {
  @override
  Future<int> createSurface() => wlr_layer.create(
        widget.layer,
        widget.anchor,
        widget.namespace,
        this,
      );

  @override
  Future<void> updateSurface(int viewId, WlrLayerSurface oldWidget) {
    if (oldWidget.namespace != widget.namespace) {
      throw Exception("The namespace of a WlrLayer cannot be changed. "
          "This is a limitation of the underlying protocol. "
          "The namespace was changed from ${oldWidget.namespace} to ${widget.namespace}. "
          "Please create a new WlrLayerSurface instead. You can do so by changing the key");
    }

    if (oldWidget.layer != widget.layer ||
        oldWidget.anchor != widget.anchor ||
        oldWidget.namespace != widget.namespace) {
      return wlr_layer.update(
        viewId,
        0,
        0,
        widget.layer,
        widget.anchor,
      );
    } else {
      return Future.value();
    }
  }

  @override
  Future<void> removeSurface(int viewId) => wlr_layer.remove(viewId);

  @override
  Widget buildView(BuildContext context, FlutterView view) => View(
        view: view,
        child: widget.child,
      );
}
