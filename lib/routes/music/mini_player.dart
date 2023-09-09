import 'dart:ui';

import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/models/current_music.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class MiniPlayerWidget extends StatelessWidget {
  const MiniPlayerWidget({super.key});

  @override
  Widget build(BuildContext context) {
    final state = context.watch<CurrentMusicModel>().value;
    if (state.id == null) {
      return const SizedBox();
    }

    return Material(
      color: EaseColors.surface,
      child: InkWell(
        onTap: () {
          routerService.goMusicPlayer();
        },
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Row(
            children: [
              _MiniPlayerCover(),
              const SizedBox(width: 14),
              _MiniPlayerCore(),
            ],
          ),
        ),
      ),
    );
  }
}

class _MiniPlayerCover extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final state = context.watch<CurrentMusicModel>().value;
    final picture = resourceService.load(state.cover);
    if (picture == null || picture.isEmpty) {
      return const SizedBox();
    }

    return Container(
      width: 58,
      height: 58,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(10),
      ),
      clipBehavior: Clip.hardEdge,
      child: Image.memory(picture),
    );
  }
}

class _MiniPlayerCore extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final state = context.watch<CurrentMusicModel>().value;
    final rate =
        clampDouble(state.currentDurationMs / state.totalDurationMs, 0, 1);

    return Expanded(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                child: Text(
                  state.title,
                  style: const TextStyle(
                    fontSize: 16,
                    color: EaseColors.primaryText,
                    overflow: TextOverflow.ellipsis,
                  ),
                  maxLines: 1,
                ),
              ),
              const SizedBox(width: 10),
              Row(
                children: [
                  if (!state.playing)
                    EaseIconButton(
                      iconToken: EaseIconsTokens.play,
                      iconColor: EaseColors.primaryText,
                      size: EaseIconButtonSize.Small,
                      onTap: (_) {
                        bridge.scope((api) => api.resumeMusic());
                      },
                    ),
                  if (state.playing)
                    EaseIconButton(
                      iconColor: EaseColors.primaryText,
                      iconToken: EaseIconsTokens.pause,
                      size: EaseIconButtonSize.Small,
                      onTap: (_) {
                        bridge.scope((api) => api.pauseMusic());
                      },
                    ),
                  EaseIconButton(
                    iconColor: state.canPlayNext
                        ? EaseColors.primaryText
                        : EaseColors.disabled,
                    iconToken: EaseIconsTokens.playNext,
                    size: EaseIconButtonSize.Small,
                    disabled: !state.canPlayNext,
                    onTap: (_) {
                      bridge.scope((api) => api.playNextMusic());
                    },
                  ),
                  EaseIconButton(
                    iconColor: EaseColors.primaryText,
                    iconToken: EaseIconsTokens.stop,
                    size: EaseIconButtonSize.Small,
                    onTap: (_) {
                      bridge.scope((api) => api.stopMusic());
                    },
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(
                child: Container(
                  height: 5,
                  decoration: BoxDecoration(
                    color: EaseColors.light,
                    borderRadius: BorderRadius.circular(5),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: FractionallySizedBox(
                    alignment: Alignment.topLeft,
                    widthFactor: rate,
                    child: Container(
                      color: EaseColors.primaryText,
                    ),
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 3),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              Text(
                state.totalDuration,
                style: const TextStyle(
                  color: EaseColors.secondaryText,
                  fontSize: 9,
                ),
              )
            ],
          ),
        ],
      ),
    );
  }
}
