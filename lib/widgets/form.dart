import 'dart:typed_data';

import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';

class _EaseFormFieldTitle extends StatelessWidget {
  final String text;

  const _EaseFormFieldTitle({required this.text});

  @override
  Widget build(BuildContext context) {
    return Text(
      text.toUpperCase(),
      style: const TextStyle(
        fontSize: 10,
        letterSpacing: 1,
        color: EaseColors.primaryText,
      ),
    );
  }
}

class EaseFormInfo extends StatelessWidget {
  final String label;
  final Widget child;

  const EaseFormInfo({super.key, required this.label, required this.child});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _EaseFormFieldTitle(
          text: label,
        ),
        const SizedBox(height: 2),
        child,
      ],
    );
  }
}

class EaseFormText extends StatelessWidget {
  final TextEditingController controller;
  final String? Function(String?)? validator;
  final String label;
  final bool? obscureText;

  const EaseFormText({
    super.key,
    required this.controller,
    this.validator,
    required this.label,
    this.obscureText,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _EaseFormFieldTitle(
          text: label,
        ),
        const SizedBox(height: 2),
        TextFormField(
          controller: controller,
          validator: validator,
          obscureText: obscureText ?? false,
          decoration: const InputDecoration(
            filled: true,
            fillColor: EaseColors.light,
          ),
        ),
      ],
    );
  }
}

class EaseFormImage extends StatelessWidget {
  final String label;
  final Uint8List? value;
  final void Function() onTapAdd;
  final void Function() onTapClear;

  const EaseFormImage({
    super.key,
    required this.label,
    required this.value,
    required this.onTapAdd,
    required this.onTapClear,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _EaseFormFieldTitle(
          text: label,
        ),
        const SizedBox(
          height: 2,
        ),
        if (value == null)
          Material(
            color: EaseColors.light,
            borderRadius: BorderRadius.circular(6),
            child: InkWell(
              onTap: onTapAdd,
              borderRadius: BorderRadius.circular(6),
              child: const SizedBox(
                width: 80,
                height: 80,
                child: Center(
                  child: EaseIcon(
                    size: 20,
                    color: EaseColors.primaryText,
                    iconToken: EaseIconsTokens.plus,
                  ),
                ),
              ),
            ),
          ),
        if (value != null)
          SizedBox(
            width: 86,
            height: 86,
            child: Stack(
              children: [
                Positioned(
                  right: 6,
                  top: 6,
                  width: 80,
                  height: 80,
                  child: Container(
                    color: EaseColors.light,
                    child: Image.memory(value!),
                  ),
                ),
                Positioned(
                  right: 0,
                  top: 0,
                  width: 12,
                  height: 12,
                  child: Material(
                    borderRadius: BorderRadius.circular(6),
                    color: EaseColors.error,
                    child: InkWell(
                      onTap: onTapClear,
                      borderRadius: BorderRadius.circular(6),
                      child: Center(
                        child: Container(
                          width: 7,
                          height: 1,
                          color: EaseColors.surface,
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          )
      ],
    );
  }
}

class EaseFormSwitch extends StatelessWidget {
  static const double offsetX = 8;
  static const double width = 53;
  static const double handleSize = 16;
  final String label;
  final bool value;
  final void Function(bool) onChange;

  const EaseFormSwitch({
    super.key,
    required this.label,
    required this.value,
    required this.onChange,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _EaseFormFieldTitle(
          text: label,
        ),
        GestureDetector(
          onTap: () {
            onChange(!value);
          },
          child: Container(
            width: width,
            height: 28,
            decoration: BoxDecoration(
              color: value ? EaseColors.primary : EaseColors.light,
              borderRadius: BorderRadius.circular(72),
            ),
            child: Stack(
              children: [
                Positioned(
                  left: value ? width - offsetX - handleSize : offsetX,
                  top: 6,
                  child: Container(
                    width: handleSize,
                    height: handleSize,
                    decoration: BoxDecoration(
                      color:
                          value ? EaseColors.surface : EaseColors.primaryText,
                      borderRadius: BorderRadius.circular(18),
                    ),
                  ),
                )
              ],
            ),
          ),
        ),
      ],
    );
  }
}
