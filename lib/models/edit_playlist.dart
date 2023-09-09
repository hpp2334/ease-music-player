import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class EditPlaylistModel with ChangeNotifier {
  VEditPlaylistState _state = const VEditPlaylistState(
    picture: null,
    name: "",
    preparedSignal: 0,
  );

  VEditPlaylistState get value => _state;

  EditPlaylistModel() {
    bridge.editPlaylistStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
