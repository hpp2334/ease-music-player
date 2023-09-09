import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/current_music.dart';
import 'package:ease_music_player/models/time_to_pause.dart';
import 'package:ease_music_player/routes/music/player_body.dart';
import 'package:ease_music_player/routes/music/slider.dart';
import 'package:ease_music_player/routes/music/timer_dialog.dart';
import 'package:ease_music_player/services/player.service.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../models/current_music_lyric.dart';
import '../../services/router.service.dart';
import '../../widgets/e_popup_menu.dart';

class MusicPlayerPage extends StatefulWidget {
  const MusicPlayerPage({super.key});

  @override
  MusicPlayerPageState createState() => MusicPlayerPageState();
}

class MusicPlayerPageState extends State<MusicPlayerPage> {
  final GlobalKey _bodyContainerKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final state = context.watch<CurrentMusicModel>().value;
    final screenWidth = MediaQuery.of(context).size.width;
    final screenHeight = MediaQuery.of(context).size.height;
    final containerWidth = screenWidth * 0.8;
    final paddingOnly = screenWidth * 0.1;

    return EaseScreenContainer(
      hidePlayer: true,
      child: Center(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _PlayerTopbar(),
            Expanded(
              key: _bodyContainerKey,
              child: PlayerBody(
                containerWidth: containerWidth,
                screenWidth: screenWidth,
                screenHeight: screenHeight,
              ),
            ),
            Padding(
              padding: EdgeInsets.symmetric(horizontal: paddingOnly),
              child: Text(
                state.title,
                style: const TextStyle(
                  fontSize: 20,
                  color: EaseColors.primaryText,
                  overflow: TextOverflow.ellipsis,
                ),
                maxLines: 3,
              ),
            ),
            const SizedBox(height: 16),
            Padding(
              padding: EdgeInsets.symmetric(horizontal: paddingOnly),
              child: MusicSlider(
                value: state.currentDurationMs,
                durationInMS: state.totalDurationMs,
                containerWidth: containerWidth,
                onChange: (value) {
                  playerService.seek(Duration(seconds: value.toInt()));
                },
                loading: state.loading,
              ),
            ),
            Padding(
              padding: EdgeInsets.symmetric(
                horizontal: paddingOnly,
                vertical: 4,
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    state.currentDuration,
                    style: const TextStyle(
                      fontSize: 10,
                      color: EaseColors.primaryText,
                    ),
                  ),
                  Text(
                    state.totalDuration,
                    style: const TextStyle(
                      fontSize: 10,
                      color: EaseColors.primaryText,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
            _PlayerPanel(state: state),
          ],
        ),
      ),
    );
  }
}

class _PlayerTopbar extends StatelessWidget {
  const _PlayerTopbar();

  @override
  Widget build(BuildContext context) {
    final lyricState = context.watch<CurrentMusicLyricModel>().value;

    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: 23,
        vertical: 12,
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          EaseIconButton(
            iconColor: EaseColors.primaryText,
            iconToken: EaseIconsTokens.collapse,
            size: EaseIconButtonSize.Small,
            onTap: (_) {
              routerService.back();
            },
          ),
          EaseIconButton(
            iconColor: EaseColors.primaryText,
            iconToken: EaseIconsTokens.verticalMore,
            size: EaseIconButtonSize.Small,
            onTap: (context) {
              showEaseButtonMenu(context, [
                EPopupItem(
                  key: "playlist",
                  label: "To Playlist",
                  callback: () {
                    routerService.goCurrentMusicPlaylist();
                  },
                ),
                if (lyricState.loadState != LyricLoadState.Missing)
                  EPopupItem(
                    key: "lyric",
                    label: "Remove Lyric",
                    color: EaseColors.error,
                    callback: () {
                      bridge.scope((api) => api.removeCurrentMusicLyric());
                    },
                  ),
                if (lyricState.loadState == LyricLoadState.Missing)
                  EPopupItem(
                    key: "lyric",
                    label: "Add Lyric",
                    callback: () {
                      routerService.goImportLyrics();
                    },
                  ),
              ]);
            },
          ),
        ],
      ),
    );
  }
}

class _PlayerPanel extends StatelessWidget {
  const _PlayerPanel({
    required this.state,
  });

  final VCurrentMusicState state;

  @override
  Widget build(BuildContext context) {
    final playModeIcon = (() {
      switch (state.playMode) {
        case PlayMode.Single:
          return EaseIconsTokens.one;
        case PlayMode.SingleLoop:
          return EaseIconsTokens.repeatOne;
        case PlayMode.List:
          return EaseIconsTokens.segment;
        case PlayMode.ListLoop:
          return EaseIconsTokens.repeat;
      }
    })();

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 47.0),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const _TimerButton(),
          const SizedBox(width: 10),
          EaseIconButton(
            iconToken: EaseIconsTokens.playPrevious,
            iconColor: !state.canPlayPrevious
                ? EaseColors.disabled
                : EaseColors.primaryText,
            size: EaseIconButtonSize.Small,
            onTap: (_) {
              playerService.skipToPrevious();
            },
            disabled: !state.canPlayPrevious,
          ),
          const SizedBox(width: 10),
          EaseIconButton(
            iconToken:
                state.playing ? EaseIconsTokens.pause : EaseIconsTokens.play,
            iconColor: EaseColors.surface,
            color: EaseColors.primary,
            size: EaseIconButtonSize.Medium,
            onTap: (_) {
              if (state.playing) {
                playerService.pause();
              } else {
                playerService.resume();
              }
            },
          ),
          const SizedBox(width: 10),
          EaseIconButton(
            iconToken: EaseIconsTokens.playNext,
            iconColor: !state.canPlayNext
                ? EaseColors.disabled
                : EaseColors.primaryText,
            size: EaseIconButtonSize.Small,
            disabled: !state.canPlayNext,
            onTap: (_) {
              playerService.skipToNext();
            },
          ),
          const SizedBox(width: 10),
          EaseIconButton(
            iconToken: playModeIcon,
            iconColor: EaseColors.primaryText,
            size: EaseIconButtonSize.Small,
            onTap: (_) {
              playerService.updateMusicPlayModeToNext();
            },
          ),
        ],
      ),
    );
  }
}

class _TimerButton extends StatelessWidget {
  const _TimerButton();

  @override
  Widget build(BuildContext context) {
    final timeToPauseState = context.watch<TimeToPauseModel>().value;

    return EaseIconButton(
      iconToken: EaseIconsTokens.timeLapse,
      iconColor: timeToPauseState.enabled
          ? EaseColors.primary
          : EaseColors.primaryText,
      size: EaseIconButtonSize.Small,
      onTap: (context) {
        showTimeToSleepDialog(context);
      },
    );
  }
}
