import 'dart:typed_data';

import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/playlist_list.dart';
import 'package:ease_music_player/routes/playlist/create_playlist_dialog.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class PlaylistListStab extends StatelessWidget {
  const PlaylistListStab({super.key});

  @override
  Widget build(BuildContext context) {
    final state = context.watch<PlaylistListModel>().value;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: state.playlistList.isEmpty
              ? const EmptyPlaylistListWidget()
              : Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 24.0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          EaseIconButton(
                            iconColor: EaseColors.primaryText,
                            iconToken: EaseIconsTokens.plus,
                            size: EaseIconButtonSize.Small,
                            onTap: (_) {
                              showCreatePlaylistDialog(context);
                            },
                          )
                        ],
                      ),
                      const SizedBox(height: 16),
                      Expanded(
                        child: GridView.builder(
                          padding: EdgeInsets.zero,
                          shrinkWrap: true,
                          gridDelegate:
                              const SliverGridDelegateWithFixedCrossAxisCount(
                            crossAxisCount: 2,
                            childAspectRatio: 0.8,
                          ),
                          itemCount: state.playlistList.length,
                          itemBuilder: (context, index) {
                            final playlist = state.playlistList[index];
                            final musicCount = playlist.count;
                            final duration = playlist.duration;
                            final picture = playlist.picture;

                            return PlaylistListItemWidget(
                              playlist: playlist,
                              musicCount: musicCount,
                              duration: duration,
                              pictureHandle: picture,
                            );
                          },
                        ),
                      ),
                    ],
                  ),
                ),
        ),
      ],
    );
  }
}

class PlaylistListItemWidget extends StatelessWidget {
  const PlaylistListItemWidget({
    super.key,
    required this.playlist,
    required this.musicCount,
    required this.duration,
    required this.pictureHandle,
  });

  final VPlaylistAbstractItem playlist;
  final int musicCount;
  final String duration;
  final int? pictureHandle;

  @override
  Widget build(BuildContext context) {
    final picture =
        pictureHandle != null ? resourceService.load(pictureHandle!) : null;

    return Material(
      color: EaseColors.surface,
      borderRadius: BorderRadius.circular(4),
      child: InkWell(
        borderRadius: BorderRadius.circular(4),
        onTap: () {
          routerService.goPlaylist(playlist.id);
        },
        child: Center(
          child: SizedBox(
            width: 150,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                PlaylistCoverWidget(picture: picture, playlist: playlist),
                const SizedBox(
                  height: 10,
                ),
                Text(
                  playlist.title,
                  style: const TextStyle(
                    color: EaseColors.primaryText,
                    fontSize: 14,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                const SizedBox(
                  height: 6,
                ),
                Text(
                  "$musicCount musics · $duration",
                  style: const TextStyle(
                    color: EaseColors.secondaryText,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class PlaylistCoverWidget extends StatelessWidget {
  const PlaylistCoverWidget({
    super.key,
    required this.picture,
    required this.playlist,
  });

  final Uint8List? picture;
  final VPlaylistAbstractItem playlist;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(20),
      child: SizedBox(
        width: 150,
        height: 150,
        child: picture == null || picture!.isEmpty
            ? Image.asset("assets/AlbumCover.png")
            : Image.memory(
                picture!,
                fit: BoxFit.cover,
                alignment: Alignment.topCenter,
              ),
      ),
    );
  }
}

class EmptyPlaylistListWidget extends StatelessWidget {
  const EmptyPlaylistListWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: GestureDetector(
        onTap: () {
          showCreatePlaylistDialog(context);
        },
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Image.asset(
              "assets/EmptyPlaylists.png",
              width: 110,
            ),
            const SizedBox(height: 20),
            const Text(
              "Empty Playlists. Tap to add one.",
              style: TextStyle(
                color: EaseColors.primaryText,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
