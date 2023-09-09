import 'dart:async';
import 'dart:ffi';

import 'dart:io';
import 'package:ease_music_player/services/resource.service.dart';

import 'package:ease_music_player/bridge_generated.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:package_info_plus/package_info_plus.dart';

const _base = 'ease_client';

const _dylib = 'lib$_base.so';

final _api = EaseClientImpl(Platform.isIOS || Platform.isMacOS
    ? DynamicLibrary.executable()
    : DynamicLibrary.open(_dylib));

class Bridge {
  final StreamController<VCurrentPlaylistState> currentPlaylistStream =
      StreamController();
  final StreamController<VCurrentStorageEntriesState>
      currentStorageEntriesStream = StreamController();
  final StreamController<VPlaylistListState> playlistListStream =
      StreamController();
  final StreamController<VStorageListState> storageListStream =
      StreamController();
  final StreamController<VCurrentMusicState> currentMusicStream =
      StreamController();
  final StreamController<VCurrentMusicState> currentMusicForNotificationStream =
      StreamController();
  final StreamController<VTimeToPauseState> timeToPauseStream =
      StreamController();
  final StreamController<VCurrentMusicLyricState> currentMusicLyricStream =
      StreamController();
  final StreamController<VRootSubKeyState> rootSubKeyStream =
      StreamController();
  final StreamController<VEditPlaylistState> editPlaylistStream =
      StreamController();
  final StreamController<VCreatePlaylistState> createPlaylistStream =
      StreamController();
  final StreamController<VEditStorageState> editStorageStream =
      StreamController();
  String version = "unknown";

  Future<String> _getStoragePath() async {
    return "/";
  }

  void bindFlushSchedule() {
    _api.initNotifySchedule().listen((event) {
      scope((api) => api.flushSchedule());
    });
  }

  void _viewModelNotify(RootViewModelState event) async {
    if (event.currentPlaylist != null) {
      currentPlaylistStream.add(event.currentPlaylist!);
    }
    if (event.currentStorageEntries != null) {
      currentStorageEntriesStream.add(event.currentStorageEntries!);
    }
    if (event.playlistList != null) {
      playlistListStream.add(event.playlistList!);
    }
    if (event.storageList != null) {
      storageListStream.add(event.storageList!);
    }
    if (event.currentMusic != null) {
      currentMusicStream.add(event.currentMusic!);
      currentMusicForNotificationStream.add(event.currentMusic!);
    }
    if (event.timeToPause != null) {
      timeToPauseStream.add(event.timeToPause!);
    }
    if (event.currentMusicLyric != null) {
      currentMusicLyricStream.add(event.currentMusicLyric!);
    }
    if (event.currentRouter != null) {
      rootSubKeyStream.add(event.currentRouter!);
    }
    if (event.editPlaylist != null) {
      editPlaylistStream.add(event.editPlaylist!);
    }
    if (event.createPlaylist != null) {
      createPlaylistStream.add(event.createPlaylist!);
    }
    if (event.editStorage != null) {
      editStorageStream.add(event.editStorage!);
    }
  }

  Future<void> start() async {
    final Directory documentDir = await getApplicationDocumentsDirectory();
    final String documentDirPath = documentDir.path.endsWith("/")
        ? documentDir.path
        : "${documentDir.path}/";
    final String storagePath = await _getStoragePath();

    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    version = packageInfo.version;

    scope((api) => api.initializeClient(
          arg: ArgInitializeApp(
            appDocumentDir: documentDirPath,
            storagePath: storagePath,
            schemaVersion: 1,
          ),
        ));
    await syncStoragePermission();
  }

  Future<void> syncStoragePermission() async {
    final isGranted = await Permission.manageExternalStorage.isGranted;
    scope((api) => api.updateStoragePermission(arg: isGranted));
  }

  void scope(InvokeRet Function(EaseClientImpl) f) {
    final ret = f(_api);
    if (ret.view != null) {
      _viewModelNotify(ret.view!);
    }
    resourceService.onResourcesChange(ret.resources);
  }

  EaseClientImpl getRawBinding() {
    return _api;
  }
}

final bridge = Bridge();
