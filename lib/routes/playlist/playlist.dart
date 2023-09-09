import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/current_music.dart';
import 'package:ease_music_player/models/current_playlist.dart';
import 'package:ease_music_player/services/player.service.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/dialog.dart';
import 'package:ease_music_player/widgets/e_popup_menu.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'edit_playlist_dialog.dart';

class PlaylistPage extends StatefulWidget {
  const PlaylistPage({super.key});

  @override
  PlaylistPageState createState() => PlaylistPageState();
}

const double _maxDragOffsetX = 50;

class PlaylistPageState extends State<PlaylistPage> {
  MusicId? currentDragId;
  double currentDragOffsetX = 0;

  @override
  Widget build(BuildContext context) {
    const headerHeight = 157.0;

    final state = context.watch<CurrentPlaylistModel>().value;
    final currentMusicState = context.watch<CurrentMusicModel>().value;
    final musicCount = state.items.length;
    final duration = state.duration;
    final disabledPlayAll = musicCount == 0;
    final picture =
        state.picture != null ? resourceService.load(state.picture!) : null;

    var playAllButton = Positioned(
      top: 125,
      right: 30,
      width: 64,
      height: 64,
      child: EaseIconButton(
        color: disabledPlayAll ? EaseColors.disabled : EaseColors.primary,
        iconColor: EaseColors.surface,
        iconToken: EaseIconsTokens.play,
        size: EaseIconButtonSize.Medium,
        disabled: disabledPlayAll,
        onTap: (_) {
          bridge.scope((api) => api.playAllMusics());
          routerService.goMusicPlayer();
        },
      ),
    );
    return EaseScreenContainer(
      barColor: const Color.fromRGBO(0x3A, 0x3A, 0x3A, 1),
      child: Stack(
        children: [
          Positioned(
            top: 0,
            height: headerHeight,
            left: 0,
            right: 0,
            child: Stack(
              children: [
                Positioned(
                  left: 0,
                  top: 0,
                  right: 0,
                  bottom: 0,
                  child: picture == null || picture.isEmpty
                      ? Image.asset(
                          "assets/DefaultHeaderCover.png",
                          fit: BoxFit.cover,
                        )
                      : Stack(
                          children: [
                            Positioned(
                              left: 0,
                              right: 0,
                              bottom: 0,
                              top: 0,
                              child: Image.memory(
                                picture,
                                fit: BoxFit.cover,
                              ),
                            ),
                            Container(
                              color: Colors.black.withOpacity(0.5),
                            )
                          ],
                        ),
                ),
                Positioned(
                  left: 0,
                  top: 0,
                  right: 0,
                  bottom: 0,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _PlaylistTopbar(state: state),
                      Padding(
                        padding:
                            const EdgeInsets.only(left: 45, top: 9, right: 45),
                        child: Text(
                          state.title,
                          style: const TextStyle(
                            fontSize: 24,
                            overflow: TextOverflow.ellipsis,
                            fontWeight: FontWeight.bold,
                            color: Colors.white,
                          ),
                          maxLines: 1,
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(left: 45, top: 5),
                        child: Text(
                          "$musicCount musics · $duration",
                          style: const TextStyle(color: Colors.white),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            top: headerHeight,
            child: state.items.isEmpty
                ? const EmptyPlaylistWidget()
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 40),
                    itemCount: state.items.length,
                    itemBuilder: (context, index) {
                      final music = state.items[index];
                      final isCurrent = currentMusicState.id != null &&
                          currentMusicState.id!.field0 == music.id.field0;
                      return _PlaylistMusicListItem(
                        isCurrent: isCurrent,
                        music: music,
                        currentDragOffsetX:
                            currentDragId == music.id ? currentDragOffsetX : 0,
                        handleDragStart: (id, localPosX) {
                          if (id != currentDragId) {
                            setState(() {
                              currentDragId = id;
                              currentDragOffsetX = 0;
                            });
                          }
                        },
                        handleDragUpdate: (id, deltaX) {
                          if (id == currentDragId) {
                            setState(() {
                              currentDragOffsetX += deltaX;
                            });
                          }
                        },
                        handleDragEnd: (id) {
                          if (id == currentDragId) {
                            final resolvedDragOffsetX =
                                currentDragOffsetX <= -_maxDragOffsetX / 2
                                    ? -_maxDragOffsetX
                                    : 0.0;
                            setState(() {
                              currentDragOffsetX = resolvedDragOffsetX;
                            });
                          }
                        },
                        handleClearDrag: () {
                          currentDragId = null;
                        },
                      );
                    },
                  ),
          ),
          playAllButton,
        ],
      ),
    );
  }
}

class _PlaylistTopbar extends StatelessWidget {
  const _PlaylistTopbar({
    required this.state,
  });

  final VCurrentPlaylistState state;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(13),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          EaseIconButton(
            iconColor: EaseColors.surface,
            iconToken: EaseIconsTokens.back,
            size: EaseIconButtonSize.Small,
            onTap: (_) {
              routerService.back();
            },
          ),
          EaseIconButton(
            iconColor: EaseColors.surface,
            iconToken: EaseIconsTokens.verticalMore,
            size: EaseIconButtonSize.Small,
            onTap: (context) {
              showPlaylistMoreMenu(context, state.title);
            },
          ),
        ],
      ),
    );
  }

  void showPlaylistMoreMenu(BuildContext context, String playlistName) {
    return showEaseButtonMenu(context, [
      EPopupItem(
        key: "import",
        label: "Import Musics",
        callback: () {
          routerService.goImportMusics();
        },
      ),
      EPopupItem(
        key: "edit",
        label: "Edit",
        callback: () {
          showEditPlaylistDialog(
            context,
            state.id!,
          );
        },
      ),
      EPopupItem(
        key: "remove",
        label: "Remove",
        color: EaseColors.error,
        callback: () {
          final id = state.id;
          if (id != null) {
            showConfirmDialog(
              context,
              () {
                routerService.back();
                bridge.scope((api) => api.removePlaylist(arg: id));
              },
              (context) => RichText(
                text: TextSpan(children: [
                  const TextSpan(
                    text: "Are you sure to remove \"",
                    style:
                        TextStyle(fontSize: 14, color: EaseColors.primaryText),
                  ),
                  TextSpan(
                    text: playlistName,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.bold,
                      color: EaseColors.primaryText,
                    ),
                  ),
                  const TextSpan(
                    text: "\"",
                    style:
                        TextStyle(fontSize: 14, color: EaseColors.primaryText),
                  ),
                ]),
              ),
            );
          }
        },
      ),
    ]);
  }
}

class EmptyPlaylistWidget extends StatelessWidget {
  const EmptyPlaylistWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          Image.asset(
            "assets/EmptyPlaylist.png",
            width: 107,
          ),
          const SizedBox(
            height: 10,
          ),
          const Text(
            "Empty Musics in playlist",
            style: TextStyle(
              color: EaseColors.primaryText,
            ),
          ),
        ],
      ),
    );
  }
}

class _PlaylistMusicListItem extends StatelessWidget {
  const _PlaylistMusicListItem({
    required this.isCurrent,
    required this.music,
    required this.handleDragStart,
    required this.handleDragUpdate,
    required this.handleDragEnd,
    required this.handleClearDrag,
    required this.currentDragOffsetX,
  });

  final bool isCurrent;
  final VPlaylistMusicItem music;
  final void Function(MusicId, double) handleDragStart;
  final void Function(MusicId, double) handleDragUpdate;
  final void Function(MusicId) handleDragEnd;
  final void Function() handleClearDrag;
  final double currentDragOffsetX;

  static const double iconButtonHolderWidth = 40;

  double getCurrentOffsetX() {
    return clampDouble(-currentDragOffsetX, 0, _maxDragOffsetX);
  }

  @override
  Widget build(BuildContext context) {
    final currentOffsetX = getCurrentOffsetX();
    return GestureDetector(
      onHorizontalDragStart: (details) {
        handleDragStart(music.id, details.localPosition.dx);
      },
      onHorizontalDragUpdate: (details) {
        handleDragUpdate(music.id, details.delta.dx);
      },
      onHorizontalDragEnd: (details) {
        handleDragEnd(music.id);
      },
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 13),
        height: 56,
        child: Stack(
          children: [
            Positioned(
              right: currentOffsetX - iconButtonHolderWidth,
              child: Container(
                padding: const EdgeInsets.only(top: 10, bottom: 10, left: 4),
                width: iconButtonHolderWidth,
                child: EaseIconButton(
                  iconColor: EaseColors.surface,
                  iconToken: EaseIconsTokens.deleteSleep,
                  size: EaseIconButtonSize.Small,
                  color: EaseColors.error,
                  onTap: (_) {
                    bridge.scope((api) =>
                        api.removeMusicFromCurrentPlaylist(arg: music.id));
                  },
                ),
              ),
            ),
            Positioned.fill(
              left: -currentOffsetX,
              right: currentOffsetX,
              child: Material(
                color:
                    isCurrent ? EaseColors.primaryLighter : Colors.transparent,
                borderRadius: BorderRadius.circular(14),
                child: InkWell(
                  borderRadius: BorderRadius.circular(14),
                  onTap: () {
                    playerService.startPlay(music.id);
                    routerService.goMusicPlayer();
                    handleClearDrag();
                  },
                  child: Stack(
                    children: [
                      if (isCurrent)
                        const Positioned(
                          top: 38,
                          right: 4,
                          child: EaseIcon(
                            color: EaseColors.primary,
                            iconToken: EaseIconsTokens.musicNote,
                            size: 24,
                          ),
                        ),
                      Positioned.fill(
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 16, vertical: 8),
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.center,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              EaseIcon(
                                color: isCurrent
                                    ? EaseColors.primary
                                    : EaseColors.primaryText,
                                iconToken: EaseIconsTokens.musicNote,
                                size: 24,
                              ),
                              const SizedBox(
                                width: 10,
                              ),
                              Expanded(
                                child: Text(
                                  music.title,
                                  style: TextStyle(
                                    color: isCurrent
                                        ? EaseColors.primary
                                        : EaseColors.primaryText,
                                    fontSize: 14,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                  maxLines: 1,
                                ),
                              ),
                              const SizedBox(
                                width: 10,
                              ),
                              Text(
                                music.duration,
                                style: TextStyle(
                                  color: isCurrent
                                      ? EaseColors.primary
                                      : EaseColors.secondaryText,
                                  fontSize: 14,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
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
