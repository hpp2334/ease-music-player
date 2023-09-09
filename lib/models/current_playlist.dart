import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class CurrentPlaylistModel with ChangeNotifier {
  VCurrentPlaylistState _state = const VCurrentPlaylistState(
    items: [],
    duration: '',
    title: '',
  );

  VCurrentPlaylistState get value => _state;

  CurrentPlaylistModel() {
    bridge.currentPlaylistStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
