import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class EditStorageModel with ChangeNotifier {
  VEditStorageState _state = const VEditStorageState(
    isCreated: true,
    title: "",
    info: ArgUpsertStorage(
      addr: "",
      username: "",
      password: "",
      isAnonymous: false,
      typ: StorageType.Webdav,
    ),
    updateSignal: 0,
    test: StorageConnectionTestResult.None,
    musicCount: 0,
    playlistCount: 0,
  );

  VEditStorageState get value => _state;

  EditStorageModel() {
    bridge.editStorageStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
