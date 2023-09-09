import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class StorageListModel with ChangeNotifier {
  VStorageListState _state = const VStorageListState(items: []);

  VStorageListState get value => _state;

  StorageListModel() {
    bridge.storageListStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
