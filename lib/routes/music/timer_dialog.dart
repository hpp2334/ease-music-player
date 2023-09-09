import 'dart:math';

import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/time_to_pause.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/services/snackbar.service.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../widgets/button.dart';
import '../../widgets/dialog.dart';

String timeFmtTwoDigits(int x) {
  return x <= 9 ? "0$x" : "$x";
}

int _clampInt(int x, int l, int r) {
  return x < l
      ? l
      : x > r
          ? r
          : x;
}

double _clampDouble(double x, double l, double r) {
  return x < l
      ? l
      : x > r
          ? r
          : x;
}

void showTimeToSleepDialog(BuildContext context) {
  final state = context.read<TimeToPauseModel>().value;

  showDialog(
    context: context,
    builder: (ctx) {
      return TimeToSleepDialog(
        initialHour: state.leftHour,
        initialMinute: state.leftMinute,
        initialEnabled: state.enabled,
        onComplete: (hour, minute) {
          bridge.scope((api) => api.updateTimeToPause(
                arg: Duration(hours: hour, minutes: minute).inMilliseconds,
              ));
        },
        onDelete: () {
          bridge.scope((api) => api.removeTimeToPause());
        },
      );
    },
  );
}

class TimeToSleepDialog extends StatefulWidget {
  final int initialHour;
  final int initialMinute;
  final bool initialEnabled;
  final void Function(int hour, int minute) onComplete;
  final void Function() onDelete;
  const TimeToSleepDialog({
    super.key,
    required this.initialHour,
    required this.initialMinute,
    required this.initialEnabled,
    required this.onComplete,
    required this.onDelete,
  });

  @override
  TimeToSleepDialogState createState() => TimeToSleepDialogState();
}

class TimeToSleepDialogState extends State<TimeToSleepDialog> {
  static const int max = 99;
  static const titleTextStyle = TextStyle(
    color: EaseColors.primaryText,
    fontSize: 9,
  );
  int hour = 0;
  int minute = 0;

  @override
  void initState() {
    super.initState();

    hour = _clampInt(widget.initialHour, 0, 99);
    minute = _clampInt(widget.initialMinute, 0, 99);
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return EaseDialog(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const SizedBox(height: 40),
          Row(
            mainAxisAlignment: MainAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              Column(
                children: [
                  const Text(
                    "HOURS",
                    style: titleTextStyle,
                  ),
                  _ValuePicker(
                    max: 99,
                    initialValue: hour,
                    onChange: (value) {
                      hour = value;
                    },
                  ),
                ],
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const SizedBox(height: 20),
                    Container(
                      width: 0.5,
                      color: EaseColors.secondaryText,
                    ),
                    const SizedBox(height: 5),
                  ],
                ),
              ),
              Column(
                children: [
                  const Text(
                    "MINUTES",
                    style: titleTextStyle,
                  ),
                  _ValuePicker(
                    max: 59,
                    initialValue: minute,
                    onChange: (value) {
                      minute = value;
                    },
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 20),
          Padding(
            padding: const EdgeInsets.all(20.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                EaseTextButton(
                  text: "DELETE",
                  onPressed: () {
                    widget.onDelete();
                    routerService.back();
                  },
                  style: EaseTextButtonStyle.Error,
                  disabled: !widget.initialEnabled,
                ),
                Row(
                  children: [
                    EaseTextButton(
                      text: "CANCEL",
                      onPressed: routerService.back,
                    ),
                    EaseTextButton(
                      text: "OK",
                      onPressed: () {
                        if (hour == 0 && minute == 0) {
                          toastService
                              .showError("Cannot set the timer to zero.");
                          return;
                        }

                        widget.onComplete(hour, minute);
                        routerService.back();
                      },
                    ),
                  ],
                ),
              ],
            ),
          )
        ],
      ),
    );
  }
}

class _ValuePicker extends StatefulWidget {
  final int max;
  final int initialValue;
  final void Function(int) onChange;

  _ValuePicker({
    required this.max,
    required this.initialValue,
    required this.onChange,
  }) {
    assert(max >= 0 && max <= 99);
  }

  @override
  State<_ValuePicker> createState() => _ValuePickerState();
}

const _commonStyle = TextStyle(
  fontSize: 44,
  fontWeight: FontWeight.w300,
  color: EaseColors.secondaryText,
);

class _ValuePickerState extends State<_ValuePicker>
    with TickerProviderStateMixin {
  static const int scrollAnimationDuration = 300;
  static const double maxScrollSpeed = 250;
  int currentValue = 0;
  int? startCurrentValue;
  double? startDraggingOffsetY;
  double currentOffsetY = 0;
  final fontSizeTween = Tween(begin: 35.0, end: 44.0);
  final colorTween =
      ColorTween(begin: EaseColors.secondaryText, end: EaseColors.primaryText);
  static const loop = 55;
  Animation<double>? draggingEndAnimation;
  AnimationController? draggingEndAnimationController;

  void clearAnimation() {
    if (draggingEndAnimationController != null) {
      draggingEndAnimationController!.dispose();
    }
    draggingEndAnimationController = null;
    draggingEndAnimation = null;
  }

  TextStyle buildStyle(double x) => TextStyle(
        fontSize: fontSizeTween.transform(x),
        fontWeight: _commonStyle.fontWeight,
        color: colorTween.transform(x),
      );

  Widget buildItem(int i) {
    final double offsetY = (getOffsetYInUniformValue() + loop) + i * loop;
    final int value = _next(currentValue, i);

    return Positioned(
      top: offsetY,
      child: SizedBox(
        width: 60,
        height: 60,
        child: Center(
          child: Text(
            timeFmtTwoDigits(value),
            style: buildStyle(max(1.0 - (loop - offsetY).abs() / loop, 0)),
          ),
        ),
      ),
    );
  }

  int _next(int x, int offset) {
    final mod = widget.max + 1;
    int next = x + offset;
    if (offset >= 0) {
      return next % mod;
    } else {
      return ((next % mod) + mod) % mod;
    }
  }

  double getOffsetYInUniformValue() {
    final offsetY = currentOffsetY;
    return offsetY - (offsetY ~/ loop) * loop;
  }

  int calculateCurrentValue() {
    final offsetY = currentOffsetY;
    final offsetValue = -(offsetY ~/ loop);
    return _next(startCurrentValue!, offsetValue);
  }

  int calculateSnappedCurrentValue() {
    final offsetY = currentOffsetY;
    final roundedOffsetValue = (-(offsetY / loop)).round();
    return _next(startCurrentValue!, roundedOffsetValue);
  }

  double calculateSnappedOffsetY(double offsetY) {
    return (offsetY / loop).round() * loop.toDouble();
  }

  @override
  void initState() {
    super.initState();
    currentValue = widget.initialValue;
  }

  @override
  void dispose() {
    clearAnimation();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onVerticalDragStart: (e) {
        clearAnimation();
        setState(() {
          startDraggingOffsetY = e.localPosition.dy;
          startCurrentValue = currentValue;
          currentOffsetY = 0;
        });
      },
      onVerticalDragUpdate: (e) {
        currentOffsetY = e.localPosition.dy - startDraggingOffsetY!;

        setState(() {
          currentValue = calculateCurrentValue();
        });
      },
      onVerticalDragEnd: (e) {
        draggingEndAnimationController = AnimationController(
          duration: const Duration(milliseconds: scrollAnimationDuration),
          vsync: this,
        );

        draggingEndAnimation = Tween(
          begin: currentOffsetY,
          end: calculateSnappedOffsetY(currentOffsetY +
              _clampDouble(e.velocity.pixelsPerSecond.dy, -maxScrollSpeed,
                  maxScrollSpeed)),
        )
            .chain(CurveTween(curve: Curves.easeOut))
            .animate(draggingEndAnimationController!);
        draggingEndAnimationController!.addStatusListener((status) {
          if (status == AnimationStatus.completed) {
            currentValue = calculateSnappedCurrentValue();
            widget.onChange(currentValue);
            clearAnimation();

            setState(() {
              startDraggingOffsetY = null;
              startCurrentValue = null;
              currentOffsetY = 0;
            });
          }
        });
        draggingEndAnimationController!.addListener(() {
          currentOffsetY = draggingEndAnimation!.value;
          currentValue = calculateCurrentValue();
          setState(() {});
        });
        draggingEndAnimationController!.forward();
      },
      child: SizedBox(
        width: 60,
        height: 160,
        child: Stack(
          children: [
            buildItem(-2),
            buildItem(-1),
            buildItem(0),
            buildItem(1),
            buildItem(2),
          ],
        ),
      ),
    );
  }
}
