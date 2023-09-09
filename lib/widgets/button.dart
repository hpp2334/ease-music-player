import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';

enum EaseIconButtonSize {
  VerySmall,
  Small,
  Medium,
}

double _getSize(EaseIconButtonSize s) {
  switch (s) {
    case EaseIconButtonSize.VerySmall:
      return 24;
    case EaseIconButtonSize.Small:
      return 36;
    case EaseIconButtonSize.Medium:
      return 64;
  }
}

double _getIconSize(EaseIconButtonSize s) {
  switch (s) {
    case EaseIconButtonSize.VerySmall:
      return 10;
    case EaseIconButtonSize.Small:
      return 16;
    case EaseIconButtonSize.Medium:
      return 24;
  }
}

double _getPadding(EaseIconButtonSize s) {
  switch (s) {
    case EaseIconButtonSize.VerySmall:
      return 7;
    case EaseIconButtonSize.Small:
      return 10;
    case EaseIconButtonSize.Medium:
      return 20;
  }
}

class EaseIconButton extends StatelessWidget {
  final EaseIconsToken iconToken;
  final Color iconColor;
  final void Function(BuildContext) onTap;
  final void Function(BuildContext)? onLongPress;
  final EaseIconButtonSize size;
  final Color? color;
  final bool? disabled;
  const EaseIconButton({
    super.key,
    required this.iconToken,
    required this.onTap,
    required this.size,
    this.color,
    this.disabled,
    this.onLongPress,
    required this.iconColor,
  });

  @override
  Widget build(BuildContext context) {
    final size = _getSize(this.size);
    final padding = _getPadding(this.size);
    final isDisabled = disabled == true;

    Widget body = SizedBox(
      width: size,
      height: size,
      child: Padding(
        padding: EdgeInsets.all(padding),
        child: EaseIcon(
          color: iconColor,
          iconToken: iconToken,
          size: _getIconSize(this.size),
        ),
      ),
    );
    if (!isDisabled) {
      body = Material(
        color: color ?? Colors.transparent,
        borderRadius: BorderRadius.circular(size),
        child: InkWell(
          onTap: () => onTap(context),
          onLongPress: () {
            if (onLongPress != null) {
              onLongPress!(context);
            }
          },
          borderRadius: BorderRadius.circular(size),
          child: body,
        ),
      );
    } else {
      body = Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(size),
          color: color ?? Colors.transparent,
        ),
        child: body,
      );
    }

    return body;
  }
}

enum EaseTextButtonStyle {
  Primary,
  Default,
  Error,
}

class EaseTextButton extends StatelessWidget {
  final String text;
  final void Function() onPressed;
  final EaseTextButtonStyle? style;
  final bool? disabled;
  final bool? small;

  const EaseTextButton({
    super.key,
    required this.text,
    required this.onPressed,
    this.style,
    this.disabled,
    this.small,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      borderRadius: BorderRadius.circular(4),
      color: EaseColors.surface,
      child: InkWell(
        onTap: disabled == true ? null : onPressed,
        borderRadius: BorderRadius.circular(4),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
          child: Text(
            text,
            style: TextStyle(
              fontSize: small == null || small == false ? 14 : 10,
              color: disabled == true
                  ? EaseColors.disabled
                  : style == EaseTextButtonStyle.Error
                      ? EaseColors.error
                      : style == EaseTextButtonStyle.Default
                          ? EaseColors.primaryText
                          : EaseColors.primary,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
      ),
    );
  }
}
