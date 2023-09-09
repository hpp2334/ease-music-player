import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/storage_list.dart';
import 'package:ease_music_player/models/time_to_pause.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../music/timer_dialog.dart';

class _BlockTitle extends StatelessWidget {
  final String text;
  final Widget? action;
  const _BlockTitle({
    required this.text,
    this.action,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(
          text,
          style: const TextStyle(
            color: EaseColors.primary,
            fontSize: 14,
          ),
        ),
        if (action != null) action!
      ],
    );
  }
}

class _DashboardTimeToPauseBlock extends StatelessWidget {
  const _DashboardTimeToPauseBlock();

  @override
  Widget build(BuildContext context) {
    final timeToPause = context.watch<TimeToPauseModel>().value;

    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: 24,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const _BlockTitle(text: "SLEEP MODE"),
          const SizedBox(height: 10),
          Material(
            color: timeToPause.enabled
                ? EaseColors.primaryLight
                : EaseColors.light,
            borderRadius: BorderRadius.circular(15),
            child: InkWell(
              onTap: () {
                showTimeToSleepDialog(context);
              },
              borderRadius: BorderRadius.circular(15),
              child: SizedBox(
                height: 90,
                child: Padding(
                  padding: const EdgeInsets.all(25),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        "${timeFmtTwoDigits(timeToPause.leftHour)}:${timeFmtTwoDigits(timeToPause.leftMinute)}",
                        style: TextStyle(
                          fontSize: 32,
                          color: timeToPause.enabled
                              ? EaseColors.primary
                              : EaseColors.primaryText,
                        ),
                      ),
                      EaseIcon(
                        color: timeToPause.enabled
                            ? EaseColors.primary
                            : EaseColors.primaryText,
                        iconToken: EaseIconsTokens.timeLapse,
                        size: 30,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _DashboardDevicesBlock extends StatelessWidget {
  const _DashboardDevicesBlock();

  void gotoAddDevice() {
    routerService.goEditStorage(null);
  }

  @override
  Widget build(BuildContext context) {
    final storageList = context.watch<StorageListModel>().value;
    final items = storageList.items
        .where((element) => element.typ != StorageType.Local)
        .toList();

    if (items.isEmpty) {
      return Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: 24,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _BlockTitle(text: "DEVICES"),
            const SizedBox(height: 10),
            Material(
              borderRadius: BorderRadius.circular(15),
              color: EaseColors.light,
              child: InkWell(
                borderRadius: BorderRadius.circular(15),
                onTap: () {
                  gotoAddDevice();
                },
                child: const SizedBox(
                  height: 72,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      EaseIcon(
                        color: EaseColors.primaryText,
                        iconToken: EaseIconsTokens.plus,
                        size: 12,
                      ),
                      SizedBox(width: 12),
                      Text(
                        "Empty Devices",
                        style: TextStyle(
                          color: EaseColors.primaryText,
                          fontSize: 14,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: 24,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.start,
        children: [
          _BlockTitle(
            text: "DEVICES",
            action: EaseIconButton(
              iconToken: EaseIconsTokens.plus,
              iconColor: EaseColors.surface,
              color: EaseColors.primary,
              size: EaseIconButtonSize.VerySmall,
              onTap: (_) {
                gotoAddDevice();
              },
            ),
          ),
          const SizedBox(height: 10),
          Expanded(
            child: ListView.builder(
              itemCount: items.length,
              shrinkWrap: true,
              padding: EdgeInsets.zero,
              itemBuilder: (ctx, i) {
                final item = items[i];

                return Material(
                  color: Colors.transparent,
                  child: InkWell(
                    onTap: () {
                      routerService.goEditStorage(item.storageId);
                    },
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 4),
                      child: Row(
                        children: [
                          const EaseIcon(
                            color: EaseColors.primaryText,
                            iconToken: EaseIconsTokens.cloud,
                            size: 32,
                          ),
                          const SizedBox(width: 20),
                          Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            mainAxisAlignment: MainAxisAlignment.start,
                            children: [
                              Text(
                                item.name,
                                style: const TextStyle(
                                  fontSize: 14,
                                  color: EaseColors.primaryText,
                                ),
                              ),
                              Text(
                                item.subTitle,
                                style: const TextStyle(
                                  fontSize: 12,
                                  color: EaseColors.secondaryText,
                                ),
                              ),
                            ],
                          )
                        ],
                      ),
                    ),
                  ),
                );
              },
            ),
          )
        ],
      ),
    );
  }
}

class DashboardStab extends StatelessWidget {
  const DashboardStab({super.key});

  @override
  Widget build(BuildContext context) {
    return const Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(height: 50),
        _DashboardTimeToPauseBlock(),
        SizedBox(height: 50),
        Expanded(
          child: _DashboardDevicesBlock(),
        ),
      ],
    );
  }
}
