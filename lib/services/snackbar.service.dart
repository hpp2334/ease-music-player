import 'package:ease_music_player/services/bridge.service.dart';
import 'package:flutter/material.dart';

class ToastService {
  final snackbarGlobalKey = GlobalKey<ScaffoldMessengerState>();

  void initialize() {
    bridge.getRawBinding().bindToastError().listen((event) {
      showError(event);
    });
  }

  void showError(String text) {
    snackbarGlobalKey.currentState!.showSnackBar(SnackBar(
      content: Text(text),
      backgroundColor: Colors.red,
    ));
  }
}

final toastService = ToastService();
