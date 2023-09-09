import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:flutter/material.dart';

class EaseDialog extends StatelessWidget {
  final Widget child;

  const EaseDialog({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: EaseColors.surface,
      surfaceTintColor: EaseColors.surface,
      shadowColor: null,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20.0)),
      clipBehavior: Clip.hardEdge,
      child: IntrinsicHeight(
        child: SizedBox(
          width: 325,
          child: child,
        ),
      ),
    );
  }
}

void showConfirmDialog(
  BuildContext context,
  void Function() onConfirm,
  Widget Function(BuildContext) builder,
) {
  showDialog(
    context: context,
    builder: (context) {
      return EaseDialog(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                "CONFIRM",
                style: TextStyle(
                  fontSize: 14,
                  color: EaseColors.error,
                ),
              ),
              const SizedBox(height: 8),
              builder(context),
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  EaseTextButton(
                    text: "CANCEL",
                    onPressed: () {
                      Navigator.of(context).pop();
                    },
                  ),
                  EaseTextButton(
                    text: "OK",
                    onPressed: () {
                      Navigator.of(context).pop(true);
                    },
                  ),
                ],
              )
            ],
          ),
        ),
      );
    },
  ).then((value) {
    if (value == true) {
      onConfirm();
    }
  });
}
