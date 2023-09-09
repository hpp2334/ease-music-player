import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/routes/crash/crash.dart';
import 'package:ease_music_player/routes/music/player.dart';
import 'package:ease_music_player/routes/playlist/playlist.dart';
import 'package:ease_music_player/routes/root/root.dart';
import 'package:ease_music_player/routes/storage/import_from_storage_entries.dart';
import 'package:flutter/material.dart';
import 'package:page_transition/page_transition.dart';

import '../routes/storage/edit_storage.dart';

enum RouteKey {
  Root,
  Playlist,
  MusicPlayer,
  EditStorage,
  ImportMusics,
  EditPlaylistCover,
  CreatePlaylistEntries,
  CreatePlaylistCover,
  ImportLyricToCurrentMusic,
  Crash,
}

class ArgRouterNavigate {
  final RouteKey routeKey;
  ArgRouterNavigate({required this.routeKey});
}

RouteKey? getRouteKeyFromString(String? key) {
  if (key == "/") {
    return RouteKey.Root;
  }
  final keyString = key.toString();
  for (var value in RouteKey.values) {
    if (value.toString() == keyString) {
      return value;
    }
  }
  return null;
}

Route<dynamic> generateRoutes(RouteSettings settings) {
  final routeKey = getRouteKeyFromString(settings.name!);
  if (routeKey == null) {
    return MaterialPageRoute(
      builder: (context) => const Center(
        child: Text("Not found"),
      ),
    );
  }

  switch (routeKey) {
    case RouteKey.Root:
      return PageTransition(
        child: const RootScreenPage(),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.Playlist:
      return PageTransition(
        child: const PlaylistPage(),
        type: PageTransitionType.rightToLeft,
        settings: settings,
      );
    case RouteKey.MusicPlayer:
      return PageTransition(
        child: const MusicPlayerPage(),
        type: PageTransitionType.bottomToTop,
        settings: settings,
      );
    case RouteKey.EditStorage:
      return PageTransition(
        child: const EditStoragePage(),
        type: PageTransitionType.rightToLeft,
        settings: settings,
      );
    case RouteKey.ImportMusics:
      return PageTransition(
        child: const ImportFromStorageEntriesPage(
          importType: CurrentStorageImportType.Musics,
        ),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.EditPlaylistCover:
      return PageTransition(
        child: const ImportFromStorageEntriesPage(
          importType: CurrentStorageImportType.EditPlaylistCover,
        ),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.CreatePlaylistEntries:
      return PageTransition(
        child: const ImportFromStorageEntriesPage(
          importType: CurrentStorageImportType.CreatePlaylistEntries,
        ),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.CreatePlaylistCover:
      return PageTransition(
        child: const ImportFromStorageEntriesPage(
          importType: CurrentStorageImportType.CreatePlaylistCover,
        ),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.ImportLyricToCurrentMusic:
      return PageTransition(
        child: const ImportFromStorageEntriesPage(
          importType: CurrentStorageImportType.CurrentMusicLyrics,
        ),
        type: PageTransitionType.fade,
        settings: settings,
      );
    case RouteKey.Crash:
      return PageTransition(
        child: const CrashScreenPage(),
        type: PageTransitionType.fade,
        settings: settings,
      );
    default:
      return PageTransition(
        child: const Center(
          child: Text("Not found"),
        ),
        type: PageTransitionType.fade,
      );
  }
}

class RouterService {
  final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

  void goRoot() {
    _push(RouteKey.Root);
  }

  void goPlaylist(PlaylistId id) {
    bridge.scope((api) => api.changeCurrentPlaylist(arg: id));
    _push(RouteKey.Playlist);
  }

  void goCurrentMusicPlaylist() {
    bridge.scope((api) => api.changeToCurrentMusicPlaylist());
    _pushAndRemoveUntilRoot(RouteKey.Playlist);
  }

  void goMusicPlayer() {
    _push(RouteKey.MusicPlayer);
  }

  void goEditStorage(StorageId? id) {
    bridge.scope((api) => api.prepareEditStorage(arg: id));
    _push(RouteKey.EditStorage);
  }

  void goImportMusics() {
    bridge.scope((api) => api.prepareImportEntriesInCurrentPlaylist());
    _push(RouteKey.ImportMusics);
  }

  void goEditPlaylistCover() {
    bridge.scope((api) => api.prepareEditPlaylistCover());
    _push(RouteKey.EditPlaylistCover);
  }

  void goCreatePlaylistEntries() {
    bridge.scope((api) => api.prepareCreatePlaylistEntries());
    _push(RouteKey.CreatePlaylistEntries);
  }

  void goCreatePlaylistCover() {
    bridge.scope((api) => api.prepareCreatePlaylistCover());
    _push(RouteKey.CreatePlaylistCover);
  }

  void goImportLyrics() {
    bridge.scope((api) => api.prepareImportLyric());
    _push(RouteKey.ImportLyricToCurrentMusic);
  }

  void goCrash() {
    navigatorKey.currentState!
        .pushNamedAndRemoveUntil(RouteKey.Crash.toString(), (route) {
      return false;
    });
  }

  void back() {
    navigatorKey.currentState!.pop();
  }

  void _push(RouteKey routeKey) {
    navigatorKey.currentState!.pushNamed(routeKey.toString());
  }

  void _pushAndRemoveUntilRoot(RouteKey routeKey) {
    navigatorKey.currentState!.pushNamedAndRemoveUntil(routeKey.toString(),
        (route) {
      final routeKey = getRouteKeyFromString(route.settings.name);
      return routeKey != null && routeKey == RouteKey.Root;
    });
  }
}

final routerService = RouterService();
