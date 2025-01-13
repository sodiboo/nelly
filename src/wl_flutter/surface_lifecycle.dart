import "dart:ui";

import "package:flutter/widgets.dart";

mixin SurfaceLifecycle<Self extends StatefulWidget>
    on State<Self>, WidgetsBindingObserver {
  Future<int> createSurface();
  Future<void> updateSurface(int viewId, Self oldWidget);
  Future<void> removeSurface(int viewId);

  Widget buildView(BuildContext context, FlutterView view);

  int? _viewId;
  FlutterView? _view;
  bool _wasDisposed = false;
  Self? _oldWidget;

  void _tryUpdateView() {
    final viewId = _viewId;
    if (viewId != null) {
      setState(() {
        _view = PlatformDispatcher.instance.view(id: viewId);
      });
    }
  }

  void _tryUpdateSurface() {
    final oldWidget = _oldWidget;
    final viewId = _viewId;

    if (oldWidget != null && viewId != null) {
      updateSurface(viewId, oldWidget);
      _oldWidget = null;
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    createSurface().then((viewId) {
      if (_wasDisposed) {
        // very shortlived state object; close the view immediately
        removeSurface(viewId);
      } else {
        setState(() {
          _viewId = viewId;
        });
        _tryUpdateView();
        _tryUpdateSurface();
      }
    });
  }

  @override
  void didChangeMetrics() {
    _tryUpdateView();
  }

  @override
  void didUpdateWidget(covariant Self oldWidget) {
    super.didUpdateWidget(oldWidget);

    _oldWidget = oldWidget;

    _tryUpdateSurface();
  }

  @override
  void dispose() {
    _wasDisposed = true;
    final viewId = _viewId;
    if (viewId != null) {
      removeSurface(viewId);
    }

    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final view = _view;

    return (view == null)
        ? const ViewCollection(views: [])
        : buildView(context, view);
  }
}
