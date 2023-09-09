import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class PlaylistListModel with ChangeNotifier {
  VPlaylistListState _state = const VPlaylistListState(playlistList: []);

  VPlaylistListState get value => _state;

  PlaylistListModel() {
    bridge.playlistListStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
