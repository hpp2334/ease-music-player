import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class CreatePlaylistModel with ChangeNotifier {
  VCreatePlaylistState _state = const VCreatePlaylistState(
    picture: null,
    mode: CreatePlaylistMode.Full,
    musicCount: 0,
    recommendPlaylistNames: [],
    name: "",
    preparedSignal: 0,
    fullImported: false,
  );

  VCreatePlaylistState get value => _state;

  CreatePlaylistModel() {
    bridge.createPlaylistStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
