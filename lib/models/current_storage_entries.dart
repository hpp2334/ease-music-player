import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class CurrentStorageEntriesModel with ChangeNotifier {
  VCurrentStorageEntriesState _state = const VCurrentStorageEntriesState(
    entries: [],
    currentStorageId: null,
    selectedCount: 0,
    splitPaths: [],
    currentPath: "",
    stateType: CurrentStorageStateType.Loading,
    storageItems: [],
    importType: CurrentStorageImportType.Musics,
    disabledToggleAll: false,
  );

  VCurrentStorageEntriesState get value => _state;

  CurrentStorageEntriesModel() {
    bridge.currentStorageEntriesStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
