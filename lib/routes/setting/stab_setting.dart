import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';

class _SettingTitle extends StatelessWidget {
  final String title;

  const _SettingTitle({required this.title});

  @override
  Widget build(Object context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: const TextStyle(
            fontSize: 14,
            color: EaseColors.primaryText,
          ),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 6),
          child: Container(
            height: 1,
            color: EaseColors.primaryText,
          ),
        ),
      ],
    );
  }
}

class _SettingItem extends StatelessWidget {
  final EaseIconsToken iconToken;
  final String title;
  final String description;
  final void Function() onTap;

  const _SettingItem({
    required this.iconToken,
    required this.title,
    required this.description,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: EaseColors.surface,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Row(
            children: [
              EaseIcon(
                color: EaseColors.primaryText,
                iconToken: iconToken,
                size: 24,
              ),
              const SizedBox(width: 16),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontSize: 14,
                      color: EaseColors.primaryText,
                    ),
                  ),
                  Text(
                    description,
                    style: const TextStyle(
                      fontSize: 12,
                      color: EaseColors.secondaryText,
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class SettingStab extends StatelessWidget {
  const SettingStab({super.key});

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Column(
          children: [
            const SizedBox(height: 50),
            const _SettingTitle(
              title: "ABOUT",
            ),
            _SettingItem(
              iconToken: EaseIconsTokens.info,
              title: "Version",
              description: "v ${bridge.version}",
              onTap: () {},
            ),
            _SettingItem(
              iconToken: EaseIconsTokens.github,
              title: "Github Repository",
              description: "https://github.com/hpp2334/ease-music-player",
              onTap: () {},
            ),
          ],
        ),
      ),
    );
  }
}
