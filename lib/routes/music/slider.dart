import 'dart:ui';

import 'package:ease_music_player/global.dart';
import 'package:flutter/material.dart';

class MusicSlider extends StatefulWidget {
  final int value;
  final int durationInMS;
  final double containerWidth;
  final Function(double) onChange;
  final bool loading;

  static const double handleSize = 12;
  static const double extraInteractivePadding = 4;
  static const double sliderHeight = 4;

  static const double sliderContainerHeight =
      handleSize + 2 * extraInteractivePadding;
  static const double sliderOuterVerticalOnly =
      (sliderContainerHeight - sliderHeight) / 2;
  static const double handleOuterSize =
      handleSize + 2 * extraInteractivePadding;
  static const double handleOuterVerticalOnly =
      (sliderContainerHeight - handleSize) / 2;

  const MusicSlider({
    super.key,
    required this.durationInMS,
    required this.containerWidth,
    required this.value,
    required this.onChange,
    required this.loading,
  });

  @override
  State<MusicSlider> createState() => _MusicSliderState();
}

class _MusicSliderState extends State<MusicSlider> {
  double? draggingHandleValue;

  double offsetToDuration(double offset) {
    return clampDouble(offset / widget.containerWidth * widget.durationInMS, 0,
        1.0 * widget.durationInMS);
  }

  double durationToOffset(double duration) {
    return duration / widget.durationInMS * widget.containerWidth;
  }

  Color getSliderColor() {
    return widget.loading ? EaseColors.primaryLight : EaseColors.primary;
  }

  @override
  Widget build(BuildContext context) {
    double showValue =
        draggingHandleValue == null ? 1.0 * widget.value : draggingHandleValue!;

    final double handleOffsetX = widget.durationInMS <= 0
        ? 0
        : clampDouble(showValue, 0, 1.0 * widget.durationInMS) /
            widget.durationInMS *
            widget.containerWidth;

    return GestureDetector(
      onTapDown: (e) {
        widget.onChange(offsetToDuration(e.localPosition.dx));
      },
      onHorizontalDragStart: (e) {
        setState(() {
          draggingHandleValue = offsetToDuration(e.localPosition.dx);
        });
      },
      onHorizontalDragUpdate: (e) {
        setState(() {
          draggingHandleValue = offsetToDuration(e.localPosition.dx);
        });
      },
      onHorizontalDragEnd: (_) {
        widget.onChange(draggingHandleValue!);
        setState(() {
          draggingHandleValue = null;
        });
      },
      child: Container(
        height: MusicSlider.sliderContainerHeight,
        width: widget.containerWidth,
        clipBehavior: Clip.none,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            // Slider
            Positioned(
              top: 0,
              bottom: 0,
              left: 0,
              right: 0,
              child: Container(
                padding: const EdgeInsets.symmetric(
                    vertical: MusicSlider.sliderOuterVerticalOnly),
                // add color due to GestureDetector
                color: Colors.transparent,
                child: Container(
                  height: MusicSlider.sliderHeight,
                  decoration: BoxDecoration(
                    color: EaseColors.light,
                    borderRadius: BorderRadius.circular(5),
                  ),
                ),
              ),
            ),
            // Progress
            Positioned(
              top: MusicSlider.sliderOuterVerticalOnly,
              bottom: MusicSlider.sliderOuterVerticalOnly,
              left: 0,
              child: Container(
                width: handleOffsetX,
                decoration: BoxDecoration(
                  color: getSliderColor(),
                  borderRadius: BorderRadius.circular(5),
                ),
              ),
            ),
            // Handle
            Positioned(
              top: 0,
              bottom: 0,
              left: handleOffsetX - MusicSlider.handleOuterSize / 2,
              child: GestureDetector(
                onTapDown: (e) {},
                child: Container(
                  color: Colors.transparent,
                  padding:
                      const EdgeInsets.all(MusicSlider.extraInteractivePadding),
                  child: Container(
                    width: MusicSlider.handleSize,
                    height: MusicSlider.handleSize,
                    decoration: BoxDecoration(
                      color: getSliderColor(),
                      borderRadius: BorderRadius.circular(12),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
