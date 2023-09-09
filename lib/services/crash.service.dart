import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/services/services.dart';
import 'package:flutter/foundation.dart';

class CrashService {
  Future<void> initialize() async {
    await _initNativePanicCapture();
  }

  Future<void> _initNativePanicCapture() async {
    bridge.getRawBinding().initBindReportPanic().listen((arg) async {
      if (kDebugMode) {
        print(arg.info);
        print(arg.stackTrace);
      }
      routerService.goCrash();
      destroyServices();
    });
  }
}

final crashService = CrashService();
