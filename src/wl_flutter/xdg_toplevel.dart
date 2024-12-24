import "dart:ui";

import "package:flutter/widgets.dart";

import "../platform_message/xdg_toplevel.dart";
import "../tracing.dart";

class XdgToplevel extends StatefulWidget {
  final String title;
  final String appId;

  final Widget child;

  const XdgToplevel(
      {super.key,
      required this.title,
      required this.appId,
      required this.child});

  @override
  State<XdgToplevel> createState() => _XdgToplevelState();
}

class _XdgToplevelState extends State<XdgToplevel> with WidgetsBindingObserver {
  int? viewId;
  FlutterView? view;
  bool wasDisposed = false;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    info("Creating XdgToplevel for '${widget.title}'#${widget.appId}");
    createXdgToplevel(widget.title, widget.appId).then((viewId) {
      info(
          "Created XdgToplevel for '${widget.title}'#${widget.appId} as view($viewId)");
      if (wasDisposed) {
        // very shortlived state object; close the view immediately
        removeXdgToplevel(viewId);
      } else {
        setState(() {
          this.viewId = viewId;
          _updateView();
        });
      }
    });
  }

  @override
  void didChangeMetrics() {
    _updateView();
  }

  void _updateView() {
    if (viewId != null) {
      view = PlatformDispatcher.instance.view(id: viewId!);
    }
  }

  @override
  void dispose() {
    wasDisposed = true;
    if (viewId != null) {
      removeXdgToplevel(viewId!);
    }

    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (view != null) {
      return View(
        view: view!,
        child: widget.child,
      );
    } else {
      return const ViewCollection(views: []);
    }
  }
}
