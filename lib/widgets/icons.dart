import 'package:flutter/widgets.dart';
import 'package:flutter_svg/svg.dart';

class EaseIconsToken {
  final String _token;

  const EaseIconsToken(this._token);

  String value() {
    return _token;
  }
}

class EaseIconsTokens {
  static const album = EaseIconsToken("assets/icons/Album.svg");
  static const back = EaseIconsToken("assets/icons/Back.svg");
  static const collapse = EaseIconsToken("assets/icons/Collapse.svg");
  static const cloud = EaseIconsToken("assets/icons/Cloud.svg");
  static const dashboard = EaseIconsToken("assets/icons/Dashboard.svg");
  static const deleteSleep = EaseIconsToken("assets/icons/DeleteSeep.svg");
  static const download = EaseIconsToken("assets/icons/Download.svg");
  static const file = EaseIconsToken("assets/icons/File.svg");
  static const fileManaged = EaseIconsToken("assets/icons/File.svg");
  static const folder = EaseIconsToken("assets/icons/Folder.svg");
  static const forward = EaseIconsToken("assets/icons/Forward.svg");
  static const github = EaseIconsToken("assets/icons/Github.svg");
  static const info = EaseIconsToken("assets/icons/Info.svg");
  static const lyric = EaseIconsToken("assets/icons/Lyrics.svg");
  static const musicNote = EaseIconsToken("assets/icons/Music Note.svg");
  static const ok = EaseIconsToken("assets/icons/OK.svg");
  static const one = EaseIconsToken("assets/icons/One.svg");
  static const play = EaseIconsToken("assets/icons/Play.svg");
  static const playPrevious = EaseIconsToken("assets/icons/Play Previous.svg");
  static const playNext = EaseIconsToken("assets/icons/Play Next.svg");
  static const plus = EaseIconsToken("assets/icons/Plus.svg");
  static const pause = EaseIconsToken("assets/icons/Pause.svg");
  static const repeat = EaseIconsToken("assets/icons/Repeat.svg");
  static const repeatOne = EaseIconsToken("assets/icons/RepeatOne.svg");
  static const segment = EaseIconsToken("assets/icons/Segment.svg");
  static const setting = EaseIconsToken("assets/icons/Setting.svg");
  static const stop = EaseIconsToken("assets/icons/Stop.svg");
  static const timeLapse = EaseIconsToken("assets/icons/TimeLapse.svg");
  static const toggleAll = EaseIconsToken("assets/icons/ToggleAll.svg");
  static const upload = EaseIconsToken("assets/icons/Upload.svg");
  static const verticalMore = EaseIconsToken("assets/icons/VerticalMore.svg");
  static const warning = EaseIconsToken("assets/icons/Warning.svg");
  static const wifiTethering = EaseIconsToken("assets/icons/WifiTethering.svg");
}

class EaseIcon extends StatelessWidget {
  final EaseIconsToken iconToken;
  final Color color;
  final double size;

  const EaseIcon({
    super.key,
    required this.color,
    required this.iconToken,
    required this.size,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: size,
      height: size,
      child: SvgPicture.asset(
        iconToken.value(),
        theme: SvgTheme(currentColor: color),
      ),
    );
  }
}
