import 'dart:typed_data';

import 'package:ease_music_player/bridge_generated.dart';

class ResourceService {
  final Map<int, Uint8List> _store = {};

  void onResourcesChange(List<ResourceToHostAction> resources) {
    for (final resource in resources) {
      if (resource.buf == null) {
        _store.remove(resource.id);
      } else {
        _store[resource.id] = resource.buf!;
      }
    }
  }

  Uint8List? load(int id) {
    return _store[id];
  }
}

final resourceService = ResourceService();
