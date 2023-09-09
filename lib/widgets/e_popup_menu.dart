import 'package:ease_music_player/global.dart';
import 'package:flutter/material.dart';

class EPopupItem {
  final String key;
  final Icon? icon;
  final String label;
  final void Function() callback;
  final Color? color;

  EPopupItem({
    required this.key,
    this.icon,
    required this.label,
    required this.callback,
    this.color,
  });
}

void showEaseButtonMenu(
  BuildContext context,
  List<EPopupItem> list,
) {
  final RenderBox button = context.findRenderObject()! as RenderBox;
  final RenderBox overlay =
      Navigator.of(context).overlay!.context.findRenderObject()! as RenderBox;
  const Offset offset = Offset.zero;
  final RelativeRect position = RelativeRect.fromRect(
    Rect.fromPoints(
      button.localToGlobal(offset, ancestor: overlay),
      button.localToGlobal(button.size.bottomRight(Offset.zero) + offset,
          ancestor: overlay),
    ),
    Offset.zero & overlay.size,
  );

  buildTextSyle(EPopupItem e) {
    return TextStyle(
      fontSize: 14,
      color: e.color ?? EaseColors.primaryText,
      fontWeight: FontWeight.normal,
    );
  }

  final List<PopupMenuEntry<String>> items = list
      .map(
        (e) => PopupMenuItem<String>(
          value: e.key,
          height: 36,
          child: SizedBox(
            width: 100,
            child: e.icon == null
                ? Text(
                    e.label,
                    style: buildTextSyle(e),
                  )
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      e.icon!,
                      const SizedBox(
                        width: 10,
                      ),
                      Text(
                        e.label,
                        style: buildTextSyle(e),
                      )
                    ],
                  ),
          ),
        ),
      )
      .toList();
  // Only show the menu if there is something to show
  if (items.isNotEmpty) {
    showMenu<String?>(
      context: context,
      items: items,
      position: position,
      color: EaseColors.surface,
      shadowColor: null,
      surfaceTintColor: EaseColors.surface,
    ).then<void>((String? newValue) {
      if (!context.mounted) {
        return null;
      }
      if (newValue == null) {
        return null;
      }
      final item = list.where((element) => element.key == newValue).firstOrNull;
      item?.callback();
    });
  }
}
