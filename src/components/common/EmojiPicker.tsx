import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { SearchIcon, XIcon } from '../icons';

// ============================================
// Emoji Data
// ============================================

interface EmojiItem {
  emoji: string;
  name: string;
  keywords: string[];
}

interface EmojiCategory {
  id: string;
  name: string;
  icon: string;
  emojis: EmojiItem[];
}

const SKIN_TONES = [
  { modifier: '', label: 'Default', preview: '\u{1F44B}' },
  { modifier: '\u{1F3FB}', label: 'Light', preview: '\u{1F44B}\u{1F3FB}' },
  { modifier: '\u{1F3FC}', label: 'Medium-Light', preview: '\u{1F44B}\u{1F3FC}' },
  { modifier: '\u{1F3FD}', label: 'Medium', preview: '\u{1F44B}\u{1F3FD}' },
  { modifier: '\u{1F3FE}', label: 'Medium-Dark', preview: '\u{1F44B}\u{1F3FE}' },
  { modifier: '\u{1F3FF}', label: 'Dark', preview: '\u{1F44B}\u{1F3FF}' },
];

// Emojis that support skin tone modifiers
const SKIN_TONE_ELIGIBLE = new Set([
  '\u{1F44D}', '\u{1F44E}', '\u{1F44B}', '\u{270B}', '\u{1F44F}', '\u{1F64C}', '\u{1F64F}',
  '\u{1F4AA}', '\u{1F448}', '\u{1F449}', '\u{1F446}', '\u{1F447}', '\u{270C}\u{FE0F}',
  '\u{1F596}', '\u{1F918}', '\u{1F919}', '\u{1F91E}', '\u{1F91F}', '\u{1F91A}', '\u{1F91B}',
  '\u{1F91C}', '\u{1F91D}', '\u{270D}\u{FE0F}', '\u{1F485}', '\u{1F933}', '\u{1F466}',
  '\u{1F467}', '\u{1F468}', '\u{1F469}', '\u{1F474}', '\u{1F475}', '\u{1F476}', '\u{1F47C}',
  '\u{1F385}', '\u{1F936}', '\u{1F478}', '\u{1F934}', '\u{1F470}', '\u{1F935}', '\u{1F930}',
  '\u{1F931}', '\u{1F647}', '\u{1F481}', '\u{1F645}', '\u{1F646}', '\u{1F64B}', '\u{1F926}',
  '\u{1F937}', '\u{1F486}', '\u{1F487}', '\u{1F6B6}', '\u{1F3C3}', '\u{1F483}', '\u{1F57A}',
  '\u{1F9D6}', '\u{1F9D7}', '\u{1F9D8}', '\u{1F9D9}', '\u{1F9DA}', '\u{1F9DB}', '\u{1F9DC}',
  '\u{1F9DD}', '\u{1F9DE}', '\u{1F9DF}',
]);

const EMOJI_CATEGORIES: EmojiCategory[] = [
  {
    id: 'smileys',
    name: 'Smileys & Emotion',
    icon: '\u{1F600}',
    emojis: [
      { emoji: '\u{1F600}', name: 'grinning face', keywords: ['happy', 'smile', 'grin'] },
      { emoji: '\u{1F603}', name: 'grinning face with big eyes', keywords: ['happy', 'smile'] },
      { emoji: '\u{1F604}', name: 'grinning face with smiling eyes', keywords: ['happy', 'joy'] },
      { emoji: '\u{1F601}', name: 'beaming face', keywords: ['happy', 'grin', 'beam'] },
      { emoji: '\u{1F606}', name: 'grinning squinting face', keywords: ['laugh', 'happy'] },
      { emoji: '\u{1F605}', name: 'grinning face with sweat', keywords: ['nervous', 'laugh'] },
      { emoji: '\u{1F923}', name: 'rolling on floor laughing', keywords: ['lol', 'rofl', 'laugh'] },
      { emoji: '\u{1F602}', name: 'face with tears of joy', keywords: ['lol', 'laugh', 'cry'] },
      { emoji: '\u{1F642}', name: 'slightly smiling face', keywords: ['smile'] },
      { emoji: '\u{1F643}', name: 'upside down face', keywords: ['silly', 'sarcasm'] },
      { emoji: '\u{1FAE0}', name: 'melting face', keywords: ['melt', 'disappear', 'hot'] },
      { emoji: '\u{1F609}', name: 'winking face', keywords: ['wink'] },
      { emoji: '\u{1F60A}', name: 'smiling face with smiling eyes', keywords: ['blush', 'happy'] },
      { emoji: '\u{1F607}', name: 'smiling face with halo', keywords: ['angel', 'innocent'] },
      { emoji: '\u{1F970}', name: 'smiling face with hearts', keywords: ['love', 'adore'] },
      { emoji: '\u{1F60D}', name: 'heart eyes', keywords: ['love', 'crush', 'heart'] },
      { emoji: '\u{1F929}', name: 'star struck', keywords: ['wow', 'star', 'eyes'] },
      { emoji: '\u{1F618}', name: 'face blowing a kiss', keywords: ['kiss', 'love'] },
      { emoji: '\u{1F617}', name: 'kissing face', keywords: ['kiss'] },
      { emoji: '\u{1F61A}', name: 'kissing face with closed eyes', keywords: ['kiss'] },
      { emoji: '\u{1F619}', name: 'kissing face with smiling eyes', keywords: ['kiss'] },
      { emoji: '\u{1F60B}', name: 'face savoring food', keywords: ['yummy', 'delicious'] },
      { emoji: '\u{1F61B}', name: 'face with tongue', keywords: ['tongue', 'playful'] },
      { emoji: '\u{1F61C}', name: 'winking face with tongue', keywords: ['tongue', 'wink', 'playful'] },
      { emoji: '\u{1F92A}', name: 'zany face', keywords: ['crazy', 'wild'] },
      { emoji: '\u{1F61D}', name: 'squinting face with tongue', keywords: ['tongue', 'playful'] },
      { emoji: '\u{1F911}', name: 'money mouth face', keywords: ['money', 'rich'] },
      { emoji: '\u{1F917}', name: 'hugging face', keywords: ['hug', 'embrace'] },
      { emoji: '\u{1F92D}', name: 'face with hand over mouth', keywords: ['oops', 'giggle'] },
      { emoji: '\u{1F92B}', name: 'shushing face', keywords: ['quiet', 'shh', 'secret'] },
      { emoji: '\u{1F914}', name: 'thinking face', keywords: ['think', 'hmm'] },
      { emoji: '\u{1F910}', name: 'zipper mouth face', keywords: ['secret', 'quiet', 'zip'] },
      { emoji: '\u{1F928}', name: 'face with raised eyebrow', keywords: ['skeptical', 'suspicious'] },
      { emoji: '\u{1F610}', name: 'neutral face', keywords: ['meh', 'neutral'] },
      { emoji: '\u{1F611}', name: 'expressionless face', keywords: ['blank', 'expressionless'] },
      { emoji: '\u{1F636}', name: 'face without mouth', keywords: ['silent', 'speechless'] },
      { emoji: '\u{1F60F}', name: 'smirking face', keywords: ['smirk', 'flirt'] },
      { emoji: '\u{1F612}', name: 'unamused face', keywords: ['bored', 'unamused'] },
      { emoji: '\u{1F644}', name: 'face with rolling eyes', keywords: ['eyeroll', 'annoyed'] },
      { emoji: '\u{1F62C}', name: 'grimacing face', keywords: ['grimace', 'awkward'] },
      { emoji: '\u{1F62E}\u200D\u{1F4A8}', name: 'face exhaling', keywords: ['exhale', 'sigh', 'relief'] },
      { emoji: '\u{1F925}', name: 'lying face', keywords: ['lie', 'pinocchio'] },
      { emoji: '\u{1F60C}', name: 'relieved face', keywords: ['relieved', 'peaceful'] },
      { emoji: '\u{1F614}', name: 'pensive face', keywords: ['sad', 'pensive'] },
      { emoji: '\u{1F62A}', name: 'sleepy face', keywords: ['sleepy', 'tired'] },
      { emoji: '\u{1F924}', name: 'drooling face', keywords: ['drool'] },
      { emoji: '\u{1F634}', name: 'sleeping face', keywords: ['sleep', 'zzz'] },
      { emoji: '\u{1F637}', name: 'face with medical mask', keywords: ['sick', 'mask'] },
      { emoji: '\u{1F912}', name: 'face with thermometer', keywords: ['sick', 'fever'] },
      { emoji: '\u{1F915}', name: 'face with head bandage', keywords: ['hurt', 'injured'] },
      { emoji: '\u{1F922}', name: 'nauseated face', keywords: ['sick', 'nauseous', 'gross'] },
      { emoji: '\u{1F92E}', name: 'face vomiting', keywords: ['sick', 'puke'] },
      { emoji: '\u{1F927}', name: 'sneezing face', keywords: ['sneeze', 'sick'] },
      { emoji: '\u{1F975}', name: 'hot face', keywords: ['hot', 'sweating'] },
      { emoji: '\u{1F976}', name: 'cold face', keywords: ['cold', 'freezing'] },
      { emoji: '\u{1F974}', name: 'woozy face', keywords: ['dizzy', 'drunk'] },
      { emoji: '\u{1F635}', name: 'face with crossed out eyes', keywords: ['dizzy', 'dead'] },
      { emoji: '\u{1F92F}', name: 'exploding head', keywords: ['mind blown', 'shocked'] },
      { emoji: '\u{1F920}', name: 'cowboy hat face', keywords: ['cowboy'] },
      { emoji: '\u{1F973}', name: 'partying face', keywords: ['party', 'celebrate'] },
      { emoji: '\u{1F978}', name: 'disguised face', keywords: ['disguise', 'spy'] },
      { emoji: '\u{1F60E}', name: 'smiling face with sunglasses', keywords: ['cool', 'sunglasses'] },
      { emoji: '\u{1F913}', name: 'nerd face', keywords: ['nerd', 'geek'] },
      { emoji: '\u{1F9D0}', name: 'face with monocle', keywords: ['monocle', 'classy', 'hmm'] },
      { emoji: '\u{1F615}', name: 'confused face', keywords: ['confused'] },
      { emoji: '\u{1FAE4}', name: 'face with diagonal mouth', keywords: ['unsure', 'skeptical'] },
      { emoji: '\u{1F61F}', name: 'worried face', keywords: ['worried', 'nervous'] },
      { emoji: '\u{1F641}', name: 'slightly frowning face', keywords: ['frown', 'sad'] },
      { emoji: '\u{1F62E}', name: 'face with open mouth', keywords: ['surprised', 'wow'] },
      { emoji: '\u{1F62F}', name: 'hushed face', keywords: ['hushed', 'surprised'] },
      { emoji: '\u{1F632}', name: 'astonished face', keywords: ['astonished', 'shocked'] },
      { emoji: '\u{1F633}', name: 'flushed face', keywords: ['flushed', 'embarrassed'] },
      { emoji: '\u{1F97A}', name: 'pleading face', keywords: ['puppy eyes', 'please'] },
      { emoji: '\u{1F979}', name: 'face holding back tears', keywords: ['sad', 'holding tears'] },
      { emoji: '\u{1F626}', name: 'frowning face with open mouth', keywords: ['anguish'] },
      { emoji: '\u{1F627}', name: 'anguished face', keywords: ['anguish', 'pain'] },
      { emoji: '\u{1F628}', name: 'fearful face', keywords: ['fear', 'scared'] },
      { emoji: '\u{1F630}', name: 'anxious face with sweat', keywords: ['anxious', 'nervous'] },
      { emoji: '\u{1F625}', name: 'sad but relieved face', keywords: ['sad', 'relieved'] },
      { emoji: '\u{1F622}', name: 'crying face', keywords: ['cry', 'sad', 'tear'] },
      { emoji: '\u{1F62D}', name: 'loudly crying face', keywords: ['cry', 'sob', 'sad'] },
      { emoji: '\u{1F631}', name: 'face screaming in fear', keywords: ['scream', 'fear', 'horror'] },
      { emoji: '\u{1F616}', name: 'confounded face', keywords: ['confounded'] },
      { emoji: '\u{1F623}', name: 'persevering face', keywords: ['persevere', 'struggle'] },
      { emoji: '\u{1F61E}', name: 'disappointed face', keywords: ['disappointed', 'sad'] },
      { emoji: '\u{1F613}', name: 'downcast face with sweat', keywords: ['sweat', 'cold sweat'] },
      { emoji: '\u{1F629}', name: 'weary face', keywords: ['weary', 'tired'] },
      { emoji: '\u{1F62B}', name: 'tired face', keywords: ['tired', 'exhausted'] },
      { emoji: '\u{1F624}', name: 'face with steam from nose', keywords: ['angry', 'triumph'] },
      { emoji: '\u{1F621}', name: 'pouting face', keywords: ['angry', 'rage', 'mad'] },
      { emoji: '\u{1F620}', name: 'angry face', keywords: ['angry', 'mad'] },
      { emoji: '\u{1F92C}', name: 'face with symbols on mouth', keywords: ['swear', 'angry'] },
      { emoji: '\u{1F608}', name: 'smiling face with horns', keywords: ['devil', 'evil'] },
      { emoji: '\u{1F47F}', name: 'angry face with horns', keywords: ['devil', 'angry'] },
      { emoji: '\u{1F480}', name: 'skull', keywords: ['dead', 'death', 'skeleton'] },
      { emoji: '\u{1F4A9}', name: 'pile of poo', keywords: ['poop', 'poo'] },
      { emoji: '\u{1F921}', name: 'clown face', keywords: ['clown'] },
      { emoji: '\u{1F47B}', name: 'ghost', keywords: ['ghost', 'halloween'] },
      { emoji: '\u{1F47D}', name: 'alien', keywords: ['alien', 'ufo'] },
      { emoji: '\u{1F47E}', name: 'alien monster', keywords: ['alien', 'game', 'monster'] },
      { emoji: '\u{1F916}', name: 'robot', keywords: ['robot', 'bot'] },
      { emoji: '\u{1F63A}', name: 'grinning cat', keywords: ['cat', 'happy'] },
      { emoji: '\u{1F638}', name: 'grinning cat with smiling eyes', keywords: ['cat', 'happy'] },
      { emoji: '\u{1F639}', name: 'cat with tears of joy', keywords: ['cat', 'laugh'] },
      { emoji: '\u{1F63B}', name: 'smiling cat with heart eyes', keywords: ['cat', 'love'] },
      { emoji: '\u{1F63C}', name: 'cat with wry smile', keywords: ['cat', 'smirk'] },
      { emoji: '\u{1F63D}', name: 'kissing cat', keywords: ['cat', 'kiss'] },
      { emoji: '\u{1F640}', name: 'weary cat', keywords: ['cat', 'scared'] },
      { emoji: '\u{1F63F}', name: 'crying cat', keywords: ['cat', 'sad'] },
      { emoji: '\u{1F63E}', name: 'pouting cat', keywords: ['cat', 'angry'] },
    ],
  },
  {
    id: 'people',
    name: 'People & Body',
    icon: '\u{1F44B}',
    emojis: [
      { emoji: '\u{1F44B}', name: 'waving hand', keywords: ['wave', 'hello', 'hi', 'bye'] },
      { emoji: '\u{1F91A}', name: 'raised back of hand', keywords: ['hand'] },
      { emoji: '\u{1F590}\u{FE0F}', name: 'hand with fingers splayed', keywords: ['hand', 'five'] },
      { emoji: '\u{270B}', name: 'raised hand', keywords: ['hand', 'high five', 'stop'] },
      { emoji: '\u{1F596}', name: 'vulcan salute', keywords: ['spock', 'star trek'] },
      { emoji: '\u{1FAF1}', name: 'rightwards hand', keywords: ['hand', 'right'] },
      { emoji: '\u{1FAF2}', name: 'leftwards hand', keywords: ['hand', 'left'] },
      { emoji: '\u{1FAF3}', name: 'palm down hand', keywords: ['hand', 'down'] },
      { emoji: '\u{1FAF4}', name: 'palm up hand', keywords: ['hand', 'up'] },
      { emoji: '\u{1F44C}', name: 'OK hand', keywords: ['ok', 'perfect', 'good'] },
      { emoji: '\u{1F90C}', name: 'pinched fingers', keywords: ['italian', 'what'] },
      { emoji: '\u{1F90F}', name: 'pinching hand', keywords: ['small', 'tiny'] },
      { emoji: '\u{270C}\u{FE0F}', name: 'victory hand', keywords: ['peace', 'victory', 'v'] },
      { emoji: '\u{1F91E}', name: 'crossed fingers', keywords: ['luck', 'hope', 'fingers crossed'] },
      { emoji: '\u{1FAF0}', name: 'hand with index finger and thumb crossed', keywords: ['love', 'money'] },
      { emoji: '\u{1F91F}', name: 'love you gesture', keywords: ['love', 'ily'] },
      { emoji: '\u{1F918}', name: 'sign of the horns', keywords: ['rock', 'metal'] },
      { emoji: '\u{1F919}', name: 'call me hand', keywords: ['call', 'shaka'] },
      { emoji: '\u{1F448}', name: 'backhand index pointing left', keywords: ['left', 'point'] },
      { emoji: '\u{1F449}', name: 'backhand index pointing right', keywords: ['right', 'point'] },
      { emoji: '\u{1F446}', name: 'backhand index pointing up', keywords: ['up', 'point'] },
      { emoji: '\u{1F447}', name: 'backhand index pointing down', keywords: ['down', 'point'] },
      { emoji: '\u{261D}\u{FE0F}', name: 'index pointing up', keywords: ['point', 'up'] },
      { emoji: '\u{1FAF5}', name: 'index pointing at the viewer', keywords: ['point', 'you'] },
      { emoji: '\u{1F44D}', name: 'thumbs up', keywords: ['like', 'yes', 'good', 'ok', 'approve'] },
      { emoji: '\u{1F44E}', name: 'thumbs down', keywords: ['dislike', 'no', 'bad', 'disapprove'] },
      { emoji: '\u{270A}', name: 'raised fist', keywords: ['fist', 'power'] },
      { emoji: '\u{1F44A}', name: 'oncoming fist', keywords: ['punch', 'fist bump'] },
      { emoji: '\u{1F91B}', name: 'left-facing fist', keywords: ['fist'] },
      { emoji: '\u{1F91C}', name: 'right-facing fist', keywords: ['fist'] },
      { emoji: '\u{1F44F}', name: 'clapping hands', keywords: ['clap', 'applause', 'bravo'] },
      { emoji: '\u{1F64C}', name: 'raising hands', keywords: ['hooray', 'praise', 'celebrate'] },
      { emoji: '\u{1FAF6}', name: 'heart hands', keywords: ['love', 'heart'] },
      { emoji: '\u{1F450}', name: 'open hands', keywords: ['hands'] },
      { emoji: '\u{1F932}', name: 'palms up together', keywords: ['prayer'] },
      { emoji: '\u{1F91D}', name: 'handshake', keywords: ['deal', 'agree', 'shake'] },
      { emoji: '\u{1F64F}', name: 'folded hands', keywords: ['pray', 'please', 'thanks', 'hope'] },
      { emoji: '\u{270D}\u{FE0F}', name: 'writing hand', keywords: ['write'] },
      { emoji: '\u{1F485}', name: 'nail polish', keywords: ['nails', 'beauty'] },
      { emoji: '\u{1F933}', name: 'selfie', keywords: ['selfie', 'phone'] },
      { emoji: '\u{1F4AA}', name: 'flexed biceps', keywords: ['strong', 'muscle', 'flex'] },
      { emoji: '\u{1F9BE}', name: 'mechanical arm', keywords: ['robot', 'prosthetic'] },
      { emoji: '\u{1F9BF}', name: 'mechanical leg', keywords: ['robot', 'prosthetic'] },
      { emoji: '\u{1F9B5}', name: 'leg', keywords: ['leg', 'kick'] },
      { emoji: '\u{1F9B6}', name: 'foot', keywords: ['foot', 'kick'] },
      { emoji: '\u{1F442}', name: 'ear', keywords: ['ear', 'listen', 'hear'] },
      { emoji: '\u{1F443}', name: 'nose', keywords: ['nose', 'smell'] },
      { emoji: '\u{1F9E0}', name: 'brain', keywords: ['brain', 'smart', 'think'] },
      { emoji: '\u{1F441}\u{FE0F}', name: 'eye', keywords: ['eye', 'see', 'look'] },
      { emoji: '\u{1F440}', name: 'eyes', keywords: ['eyes', 'see', 'look', 'watch'] },
      { emoji: '\u{1F445}', name: 'tongue', keywords: ['tongue', 'taste'] },
      { emoji: '\u{1F444}', name: 'mouth', keywords: ['mouth', 'lips'] },
      { emoji: '\u{1F476}', name: 'baby', keywords: ['baby', 'infant'] },
      { emoji: '\u{1F466}', name: 'boy', keywords: ['boy', 'child'] },
      { emoji: '\u{1F467}', name: 'girl', keywords: ['girl', 'child'] },
      { emoji: '\u{1F468}', name: 'man', keywords: ['man', 'male'] },
      { emoji: '\u{1F469}', name: 'woman', keywords: ['woman', 'female'] },
      { emoji: '\u{1F474}', name: 'old man', keywords: ['old', 'elderly', 'grandpa'] },
      { emoji: '\u{1F475}', name: 'old woman', keywords: ['old', 'elderly', 'grandma'] },
    ],
  },
  {
    id: 'animals',
    name: 'Animals & Nature',
    icon: '\u{1F43E}',
    emojis: [
      { emoji: '\u{1F435}', name: 'monkey face', keywords: ['monkey'] },
      { emoji: '\u{1F412}', name: 'monkey', keywords: ['monkey'] },
      { emoji: '\u{1F98D}', name: 'gorilla', keywords: ['gorilla', 'ape'] },
      { emoji: '\u{1F9A7}', name: 'orangutan', keywords: ['orangutan'] },
      { emoji: '\u{1F436}', name: 'dog face', keywords: ['dog', 'puppy'] },
      { emoji: '\u{1F415}', name: 'dog', keywords: ['dog'] },
      { emoji: '\u{1F429}', name: 'poodle', keywords: ['dog', 'poodle'] },
      { emoji: '\u{1F43A}', name: 'wolf', keywords: ['wolf'] },
      { emoji: '\u{1F98A}', name: 'fox', keywords: ['fox'] },
      { emoji: '\u{1F99D}', name: 'raccoon', keywords: ['raccoon'] },
      { emoji: '\u{1F431}', name: 'cat face', keywords: ['cat', 'kitten'] },
      { emoji: '\u{1F408}', name: 'cat', keywords: ['cat'] },
      { emoji: '\u{1F981}', name: 'lion', keywords: ['lion'] },
      { emoji: '\u{1F42F}', name: 'tiger face', keywords: ['tiger'] },
      { emoji: '\u{1F406}', name: 'leopard', keywords: ['leopard'] },
      { emoji: '\u{1F434}', name: 'horse face', keywords: ['horse'] },
      { emoji: '\u{1F984}', name: 'unicorn', keywords: ['unicorn', 'magic'] },
      { emoji: '\u{1F993}', name: 'zebra', keywords: ['zebra'] },
      { emoji: '\u{1F98C}', name: 'deer', keywords: ['deer'] },
      { emoji: '\u{1F42E}', name: 'cow face', keywords: ['cow'] },
      { emoji: '\u{1F437}', name: 'pig face', keywords: ['pig'] },
      { emoji: '\u{1F43D}', name: 'pig nose', keywords: ['pig'] },
      { emoji: '\u{1F411}', name: 'ewe', keywords: ['sheep'] },
      { emoji: '\u{1F410}', name: 'goat', keywords: ['goat'] },
      { emoji: '\u{1F42A}', name: 'camel', keywords: ['camel'] },
      { emoji: '\u{1F992}', name: 'giraffe', keywords: ['giraffe'] },
      { emoji: '\u{1F418}', name: 'elephant', keywords: ['elephant'] },
      { emoji: '\u{1F98F}', name: 'rhinoceros', keywords: ['rhino'] },
      { emoji: '\u{1F99B}', name: 'hippopotamus', keywords: ['hippo'] },
      { emoji: '\u{1F42D}', name: 'mouse face', keywords: ['mouse'] },
      { emoji: '\u{1F430}', name: 'rabbit face', keywords: ['rabbit', 'bunny'] },
      { emoji: '\u{1F43F}\u{FE0F}', name: 'chipmunk', keywords: ['chipmunk', 'squirrel'] },
      { emoji: '\u{1F994}', name: 'hedgehog', keywords: ['hedgehog'] },
      { emoji: '\u{1F987}', name: 'bat', keywords: ['bat', 'vampire'] },
      { emoji: '\u{1F43B}', name: 'bear', keywords: ['bear'] },
      { emoji: '\u{1F428}', name: 'koala', keywords: ['koala'] },
      { emoji: '\u{1F43C}', name: 'panda', keywords: ['panda'] },
      { emoji: '\u{1F9A5}', name: 'sloth', keywords: ['sloth', 'lazy'] },
      { emoji: '\u{1F9A6}', name: 'otter', keywords: ['otter'] },
      { emoji: '\u{1F9A8}', name: 'skunk', keywords: ['skunk'] },
      { emoji: '\u{1F998}', name: 'kangaroo', keywords: ['kangaroo'] },
      { emoji: '\u{1F9A1}', name: 'badger', keywords: ['badger'] },
      { emoji: '\u{1F43E}', name: 'paw prints', keywords: ['paw', 'pet'] },
      { emoji: '\u{1F414}', name: 'chicken', keywords: ['chicken'] },
      { emoji: '\u{1F426}', name: 'bird', keywords: ['bird'] },
      { emoji: '\u{1F427}', name: 'penguin', keywords: ['penguin'] },
      { emoji: '\u{1F54A}\u{FE0F}', name: 'dove', keywords: ['dove', 'peace'] },
      { emoji: '\u{1F985}', name: 'eagle', keywords: ['eagle', 'america'] },
      { emoji: '\u{1F986}', name: 'duck', keywords: ['duck'] },
      { emoji: '\u{1F989}', name: 'owl', keywords: ['owl', 'wise'] },
      { emoji: '\u{1F9A9}', name: 'flamingo', keywords: ['flamingo'] },
      { emoji: '\u{1F438}', name: 'frog', keywords: ['frog'] },
      { emoji: '\u{1F40D}', name: 'snake', keywords: ['snake'] },
      { emoji: '\u{1F422}', name: 'turtle', keywords: ['turtle', 'slow'] },
      { emoji: '\u{1F40A}', name: 'crocodile', keywords: ['crocodile'] },
      { emoji: '\u{1F995}', name: 'dinosaur', keywords: ['dinosaur'] },
      { emoji: '\u{1F996}', name: 't-rex', keywords: ['dinosaur', 'trex'] },
      { emoji: '\u{1F433}', name: 'whale', keywords: ['whale'] },
      { emoji: '\u{1F42C}', name: 'dolphin', keywords: ['dolphin'] },
      { emoji: '\u{1F41F}', name: 'fish', keywords: ['fish'] },
      { emoji: '\u{1F420}', name: 'tropical fish', keywords: ['fish'] },
      { emoji: '\u{1F421}', name: 'blowfish', keywords: ['fish'] },
      { emoji: '\u{1F419}', name: 'octopus', keywords: ['octopus'] },
      { emoji: '\u{1F41A}', name: 'spiral shell', keywords: ['shell'] },
      { emoji: '\u{1F40C}', name: 'snail', keywords: ['snail', 'slow'] },
      { emoji: '\u{1F98B}', name: 'butterfly', keywords: ['butterfly'] },
      { emoji: '\u{1F41B}', name: 'bug', keywords: ['bug', 'insect'] },
      { emoji: '\u{1F41C}', name: 'ant', keywords: ['ant'] },
      { emoji: '\u{1F41D}', name: 'honeybee', keywords: ['bee', 'honey'] },
      { emoji: '\u{1F41E}', name: 'lady beetle', keywords: ['ladybug'] },
      { emoji: '\u{1F490}', name: 'bouquet', keywords: ['flowers', 'bouquet'] },
      { emoji: '\u{1F338}', name: 'cherry blossom', keywords: ['flower', 'spring'] },
      { emoji: '\u{1F339}', name: 'rose', keywords: ['rose', 'flower', 'love'] },
      { emoji: '\u{1F33B}', name: 'sunflower', keywords: ['sunflower'] },
      { emoji: '\u{1F33C}', name: 'blossom', keywords: ['flower'] },
      { emoji: '\u{1F337}', name: 'tulip', keywords: ['tulip', 'flower'] },
      { emoji: '\u{1F332}', name: 'evergreen tree', keywords: ['tree', 'pine'] },
      { emoji: '\u{1F333}', name: 'deciduous tree', keywords: ['tree'] },
      { emoji: '\u{1F334}', name: 'palm tree', keywords: ['palm', 'tropical'] },
      { emoji: '\u{1F335}', name: 'cactus', keywords: ['cactus', 'desert'] },
      { emoji: '\u{1F340}', name: 'four leaf clover', keywords: ['lucky', 'clover'] },
      { emoji: '\u{1F341}', name: 'maple leaf', keywords: ['leaf', 'fall', 'autumn'] },
      { emoji: '\u{1F342}', name: 'fallen leaf', keywords: ['leaf', 'fall', 'autumn'] },
      { emoji: '\u{1F343}', name: 'leaf fluttering in wind', keywords: ['leaf', 'wind'] },
    ],
  },
  {
    id: 'food',
    name: 'Food & Drink',
    icon: '\u{1F354}',
    emojis: [
      { emoji: '\u{1F347}', name: 'grapes', keywords: ['grapes', 'fruit'] },
      { emoji: '\u{1F348}', name: 'melon', keywords: ['melon', 'fruit'] },
      { emoji: '\u{1F349}', name: 'watermelon', keywords: ['watermelon', 'fruit'] },
      { emoji: '\u{1F34A}', name: 'tangerine', keywords: ['orange', 'fruit'] },
      { emoji: '\u{1F34B}', name: 'lemon', keywords: ['lemon', 'fruit'] },
      { emoji: '\u{1F34C}', name: 'banana', keywords: ['banana', 'fruit'] },
      { emoji: '\u{1F34D}', name: 'pineapple', keywords: ['pineapple', 'fruit'] },
      { emoji: '\u{1F96D}', name: 'mango', keywords: ['mango', 'fruit'] },
      { emoji: '\u{1F34E}', name: 'red apple', keywords: ['apple', 'fruit'] },
      { emoji: '\u{1F34F}', name: 'green apple', keywords: ['apple', 'fruit'] },
      { emoji: '\u{1F350}', name: 'pear', keywords: ['pear', 'fruit'] },
      { emoji: '\u{1F351}', name: 'peach', keywords: ['peach', 'fruit'] },
      { emoji: '\u{1F352}', name: 'cherries', keywords: ['cherry', 'fruit'] },
      { emoji: '\u{1F353}', name: 'strawberry', keywords: ['strawberry', 'fruit'] },
      { emoji: '\u{1FAD0}', name: 'blueberries', keywords: ['blueberry', 'fruit'] },
      { emoji: '\u{1F95D}', name: 'kiwi fruit', keywords: ['kiwi', 'fruit'] },
      { emoji: '\u{1F345}', name: 'tomato', keywords: ['tomato'] },
      { emoji: '\u{1F965}', name: 'coconut', keywords: ['coconut'] },
      { emoji: '\u{1F951}', name: 'avocado', keywords: ['avocado'] },
      { emoji: '\u{1F346}', name: 'eggplant', keywords: ['eggplant', 'aubergine'] },
      { emoji: '\u{1F955}', name: 'carrot', keywords: ['carrot'] },
      { emoji: '\u{1F33D}', name: 'ear of corn', keywords: ['corn'] },
      { emoji: '\u{1F336}\u{FE0F}', name: 'hot pepper', keywords: ['pepper', 'spicy'] },
      { emoji: '\u{1F952}', name: 'cucumber', keywords: ['cucumber'] },
      { emoji: '\u{1F96C}', name: 'leafy green', keywords: ['lettuce', 'salad'] },
      { emoji: '\u{1F966}', name: 'broccoli', keywords: ['broccoli'] },
      { emoji: '\u{1F344}', name: 'mushroom', keywords: ['mushroom'] },
      { emoji: '\u{1F35E}', name: 'bread', keywords: ['bread'] },
      { emoji: '\u{1F950}', name: 'croissant', keywords: ['croissant', 'french'] },
      { emoji: '\u{1F956}', name: 'baguette bread', keywords: ['baguette', 'french'] },
      { emoji: '\u{1F968}', name: 'pretzel', keywords: ['pretzel'] },
      { emoji: '\u{1F9C0}', name: 'cheese wedge', keywords: ['cheese'] },
      { emoji: '\u{1F356}', name: 'meat on bone', keywords: ['meat'] },
      { emoji: '\u{1F357}', name: 'poultry leg', keywords: ['chicken', 'meat'] },
      { emoji: '\u{1F969}', name: 'cut of meat', keywords: ['steak', 'meat'] },
      { emoji: '\u{1F953}', name: 'bacon', keywords: ['bacon'] },
      { emoji: '\u{1F354}', name: 'hamburger', keywords: ['burger', 'food'] },
      { emoji: '\u{1F35F}', name: 'french fries', keywords: ['fries', 'chips'] },
      { emoji: '\u{1F355}', name: 'pizza', keywords: ['pizza'] },
      { emoji: '\u{1F32D}', name: 'hot dog', keywords: ['hotdog'] },
      { emoji: '\u{1F96A}', name: 'sandwich', keywords: ['sandwich'] },
      { emoji: '\u{1F32E}', name: 'taco', keywords: ['taco', 'mexican'] },
      { emoji: '\u{1F32F}', name: 'burrito', keywords: ['burrito', 'mexican'] },
      { emoji: '\u{1F959}', name: 'stuffed flatbread', keywords: ['pita', 'falafel'] },
      { emoji: '\u{1F9C6}', name: 'falafel', keywords: ['falafel'] },
      { emoji: '\u{1F95A}', name: 'egg', keywords: ['egg'] },
      { emoji: '\u{1F373}', name: 'cooking', keywords: ['egg', 'frying'] },
      { emoji: '\u{1F372}', name: 'pot of food', keywords: ['stew', 'soup'] },
      { emoji: '\u{1F35C}', name: 'steaming bowl', keywords: ['ramen', 'noodles'] },
      { emoji: '\u{1F363}', name: 'sushi', keywords: ['sushi', 'japanese'] },
      { emoji: '\u{1F371}', name: 'bento box', keywords: ['bento', 'japanese'] },
      { emoji: '\u{1F35B}', name: 'curry rice', keywords: ['curry', 'rice'] },
      { emoji: '\u{1F35A}', name: 'cooked rice', keywords: ['rice'] },
      { emoji: '\u{1F358}', name: 'rice cracker', keywords: ['rice'] },
      { emoji: '\u{1F370}', name: 'shortcake', keywords: ['cake', 'dessert'] },
      { emoji: '\u{1F382}', name: 'birthday cake', keywords: ['cake', 'birthday'] },
      { emoji: '\u{1F967}', name: 'pie', keywords: ['pie', 'dessert'] },
      { emoji: '\u{1F36B}', name: 'chocolate bar', keywords: ['chocolate'] },
      { emoji: '\u{1F36C}', name: 'candy', keywords: ['candy', 'sweet'] },
      { emoji: '\u{1F36D}', name: 'lollipop', keywords: ['lollipop', 'candy'] },
      { emoji: '\u{1F36E}', name: 'custard', keywords: ['custard', 'pudding'] },
      { emoji: '\u{1F36F}', name: 'honey pot', keywords: ['honey'] },
      { emoji: '\u{1F37C}', name: 'baby bottle', keywords: ['baby', 'milk'] },
      { emoji: '\u{2615}', name: 'hot beverage', keywords: ['coffee', 'tea', 'hot'] },
      { emoji: '\u{1F375}', name: 'teacup without handle', keywords: ['tea'] },
      { emoji: '\u{1F376}', name: 'sake', keywords: ['sake', 'japanese'] },
      { emoji: '\u{1F37E}', name: 'bottle with popping cork', keywords: ['champagne', 'celebrate'] },
      { emoji: '\u{1F377}', name: 'wine glass', keywords: ['wine'] },
      { emoji: '\u{1F378}', name: 'cocktail glass', keywords: ['cocktail', 'drink'] },
      { emoji: '\u{1F379}', name: 'tropical drink', keywords: ['tropical', 'drink'] },
      { emoji: '\u{1F37A}', name: 'beer mug', keywords: ['beer'] },
      { emoji: '\u{1F37B}', name: 'clinking beer mugs', keywords: ['beer', 'cheers'] },
      { emoji: '\u{1F942}', name: 'clinking glasses', keywords: ['cheers', 'toast'] },
      { emoji: '\u{1F943}', name: 'tumbler glass', keywords: ['whiskey', 'drink'] },
      { emoji: '\u{1F964}', name: 'cup with straw', keywords: ['drink', 'soda'] },
      { emoji: '\u{1F9CB}', name: 'bubble tea', keywords: ['boba', 'tea'] },
      { emoji: '\u{1F9C3}', name: 'beverage box', keywords: ['juice'] },
      { emoji: '\u{1F9C9}', name: 'mate', keywords: ['mate', 'tea'] },
      { emoji: '\u{1F9CA}', name: 'ice', keywords: ['ice', 'cube'] },
    ],
  },
  {
    id: 'activities',
    name: 'Activities',
    icon: '\u{26BD}',
    emojis: [
      { emoji: '\u{26BD}', name: 'soccer ball', keywords: ['soccer', 'football'] },
      { emoji: '\u{1F3C0}', name: 'basketball', keywords: ['basketball'] },
      { emoji: '\u{1F3C8}', name: 'american football', keywords: ['football'] },
      { emoji: '\u{26BE}', name: 'baseball', keywords: ['baseball'] },
      { emoji: '\u{1F94E}', name: 'softball', keywords: ['softball'] },
      { emoji: '\u{1F3BE}', name: 'tennis', keywords: ['tennis'] },
      { emoji: '\u{1F3D0}', name: 'volleyball', keywords: ['volleyball'] },
      { emoji: '\u{1F3C9}', name: 'rugby football', keywords: ['rugby'] },
      { emoji: '\u{1F94F}', name: 'flying disc', keywords: ['frisbee'] },
      { emoji: '\u{1F3B1}', name: 'pool 8 ball', keywords: ['billiards', 'pool'] },
      { emoji: '\u{1F3D3}', name: 'ping pong', keywords: ['ping pong', 'table tennis'] },
      { emoji: '\u{1F3F8}', name: 'badminton', keywords: ['badminton'] },
      { emoji: '\u{1F3D2}', name: 'ice hockey', keywords: ['hockey'] },
      { emoji: '\u{1F3D1}', name: 'field hockey', keywords: ['hockey'] },
      { emoji: '\u{1F94D}', name: 'lacrosse', keywords: ['lacrosse'] },
      { emoji: '\u{26F3}', name: 'flag in hole', keywords: ['golf'] },
      { emoji: '\u{1F3AF}', name: 'bullseye', keywords: ['target', 'dart'] },
      { emoji: '\u{1F3A3}', name: 'fishing pole', keywords: ['fishing'] },
      { emoji: '\u{1F93F}', name: 'diving mask', keywords: ['diving', 'snorkel'] },
      { emoji: '\u{1F3BD}', name: 'running shirt', keywords: ['marathon', 'running'] },
      { emoji: '\u{1F3BF}', name: 'skis', keywords: ['skiing'] },
      { emoji: '\u{1F6F7}', name: 'sled', keywords: ['sled', 'winter'] },
      { emoji: '\u{1F94C}', name: 'curling stone', keywords: ['curling'] },
      { emoji: '\u{1FA80}', name: 'yo-yo', keywords: ['yoyo'] },
      { emoji: '\u{1FA81}', name: 'kite', keywords: ['kite'] },
      { emoji: '\u{1F3B0}', name: 'slot machine', keywords: ['casino', 'gambling'] },
      { emoji: '\u{1F3B2}', name: 'game die', keywords: ['dice'] },
      { emoji: '\u{1F9E9}', name: 'puzzle piece', keywords: ['puzzle'] },
      { emoji: '\u{1F3AE}', name: 'video game', keywords: ['game', 'controller'] },
      { emoji: '\u{1F579}\u{FE0F}', name: 'joystick', keywords: ['game', 'joystick'] },
      { emoji: '\u{1F3B3}', name: 'bowling', keywords: ['bowling'] },
      { emoji: '\u{265F}\u{FE0F}', name: 'chess pawn', keywords: ['chess'] },
      { emoji: '\u{1F3AD}', name: 'performing arts', keywords: ['theater', 'drama'] },
      { emoji: '\u{1F3A8}', name: 'artist palette', keywords: ['art', 'paint'] },
      { emoji: '\u{1F3B5}', name: 'musical note', keywords: ['music', 'note'] },
      { emoji: '\u{1F3B6}', name: 'musical notes', keywords: ['music', 'notes'] },
      { emoji: '\u{1F3B9}', name: 'musical keyboard', keywords: ['piano', 'keyboard'] },
      { emoji: '\u{1F3B7}', name: 'saxophone', keywords: ['saxophone', 'jazz'] },
      { emoji: '\u{1FA97}', name: 'accordion', keywords: ['accordion'] },
      { emoji: '\u{1F3B8}', name: 'guitar', keywords: ['guitar', 'rock'] },
      { emoji: '\u{1F3BA}', name: 'trumpet', keywords: ['trumpet'] },
      { emoji: '\u{1F3BB}', name: 'violin', keywords: ['violin'] },
      { emoji: '\u{1FA95}', name: 'banjo', keywords: ['banjo'] },
      { emoji: '\u{1F941}', name: 'drum', keywords: ['drum'] },
      { emoji: '\u{1F3AC}', name: 'clapper board', keywords: ['movie', 'film'] },
      { emoji: '\u{1F3A4}', name: 'microphone', keywords: ['karaoke', 'mic'] },
      { emoji: '\u{1F3A7}', name: 'headphone', keywords: ['headphones', 'music'] },
    ],
  },
  {
    id: 'travel',
    name: 'Travel & Places',
    icon: '\u{2708}\u{FE0F}',
    emojis: [
      { emoji: '\u{1F30D}', name: 'globe showing Europe-Africa', keywords: ['earth', 'world'] },
      { emoji: '\u{1F30E}', name: 'globe showing Americas', keywords: ['earth', 'world'] },
      { emoji: '\u{1F30F}', name: 'globe showing Asia-Australia', keywords: ['earth', 'world'] },
      { emoji: '\u{1F30C}', name: 'milky way', keywords: ['space', 'galaxy'] },
      { emoji: '\u{2B50}', name: 'star', keywords: ['star'] },
      { emoji: '\u{1F31F}', name: 'glowing star', keywords: ['star', 'sparkle'] },
      { emoji: '\u{2728}', name: 'sparkles', keywords: ['sparkle', 'magic'] },
      { emoji: '\u{1F525}', name: 'fire', keywords: ['fire', 'hot', 'lit'] },
      { emoji: '\u{1F4A5}', name: 'collision', keywords: ['boom', 'explosion'] },
      { emoji: '\u{1F308}', name: 'rainbow', keywords: ['rainbow'] },
      { emoji: '\u{2600}\u{FE0F}', name: 'sun', keywords: ['sun', 'sunny'] },
      { emoji: '\u{1F324}\u{FE0F}', name: 'sun behind small cloud', keywords: ['partly cloudy'] },
      { emoji: '\u{26C5}', name: 'sun behind cloud', keywords: ['cloudy'] },
      { emoji: '\u{1F325}\u{FE0F}', name: 'sun behind large cloud', keywords: ['cloudy'] },
      { emoji: '\u{2601}\u{FE0F}', name: 'cloud', keywords: ['cloud'] },
      { emoji: '\u{1F327}\u{FE0F}', name: 'cloud with rain', keywords: ['rain'] },
      { emoji: '\u{26C8}\u{FE0F}', name: 'cloud with lightning and rain', keywords: ['storm'] },
      { emoji: '\u{1F329}\u{FE0F}', name: 'cloud with lightning', keywords: ['lightning'] },
      { emoji: '\u{1F328}\u{FE0F}', name: 'cloud with snow', keywords: ['snow'] },
      { emoji: '\u{2744}\u{FE0F}', name: 'snowflake', keywords: ['snow', 'winter', 'cold'] },
      { emoji: '\u{1F30A}', name: 'water wave', keywords: ['wave', 'ocean', 'sea'] },
      { emoji: '\u{1F30B}', name: 'volcano', keywords: ['volcano'] },
      { emoji: '\u{1F3D4}\u{FE0F}', name: 'snow-capped mountain', keywords: ['mountain'] },
      { emoji: '\u{26F0}\u{FE0F}', name: 'mountain', keywords: ['mountain'] },
      { emoji: '\u{1F3D6}\u{FE0F}', name: 'beach with umbrella', keywords: ['beach'] },
      { emoji: '\u{1F3DD}\u{FE0F}', name: 'desert island', keywords: ['island'] },
      { emoji: '\u{1F3DC}\u{FE0F}', name: 'desert', keywords: ['desert'] },
      { emoji: '\u{1F3D5}\u{FE0F}', name: 'camping', keywords: ['camping', 'tent'] },
      { emoji: '\u{1F3E0}', name: 'house', keywords: ['house', 'home'] },
      { emoji: '\u{1F3E2}', name: 'office building', keywords: ['office', 'building'] },
      { emoji: '\u{1F3E5}', name: 'hospital', keywords: ['hospital'] },
      { emoji: '\u{1F3EB}', name: 'school', keywords: ['school'] },
      { emoji: '\u{1F3E8}', name: 'hotel', keywords: ['hotel'] },
      { emoji: '\u{1F3F0}', name: 'castle', keywords: ['castle'] },
      { emoji: '\u{1F5FC}', name: 'Tokyo tower', keywords: ['tower', 'tokyo'] },
      { emoji: '\u{1F5FD}', name: 'Statue of Liberty', keywords: ['statue', 'liberty'] },
      { emoji: '\u{26EA}', name: 'church', keywords: ['church'] },
      { emoji: '\u{1F54C}', name: 'mosque', keywords: ['mosque'] },
      { emoji: '\u{1F680}', name: 'rocket', keywords: ['rocket', 'space', 'launch'] },
      { emoji: '\u{1F6F8}', name: 'flying saucer', keywords: ['ufo', 'alien'] },
      { emoji: '\u{2708}\u{FE0F}', name: 'airplane', keywords: ['airplane', 'flight'] },
      { emoji: '\u{1F681}', name: 'helicopter', keywords: ['helicopter'] },
      { emoji: '\u{1F682}', name: 'locomotive', keywords: ['train'] },
      { emoji: '\u{1F697}', name: 'automobile', keywords: ['car'] },
      { emoji: '\u{1F695}', name: 'taxi', keywords: ['taxi', 'cab'] },
      { emoji: '\u{1F68C}', name: 'bus', keywords: ['bus'] },
      { emoji: '\u{1F6B2}', name: 'bicycle', keywords: ['bicycle', 'bike'] },
      { emoji: '\u{1F6F4}', name: 'kick scooter', keywords: ['scooter'] },
      { emoji: '\u{1F6F5}', name: 'motor scooter', keywords: ['scooter', 'vespa'] },
      { emoji: '\u{1F6A2}', name: 'ship', keywords: ['ship', 'boat'] },
      { emoji: '\u{26F5}', name: 'sailboat', keywords: ['sailboat', 'sail'] },
    ],
  },
  {
    id: 'objects',
    name: 'Objects',
    icon: '\u{1F4A1}',
    emojis: [
      { emoji: '\u{2764}\u{FE0F}', name: 'red heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F9E1}', name: 'orange heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F49B}', name: 'yellow heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F49A}', name: 'green heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F499}', name: 'blue heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F49C}', name: 'purple heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F5A4}', name: 'black heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F90D}', name: 'white heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F90E}', name: 'brown heart', keywords: ['heart', 'love'] },
      { emoji: '\u{1F494}', name: 'broken heart', keywords: ['heart', 'break', 'sad'] },
      { emoji: '\u{1F495}', name: 'two hearts', keywords: ['hearts', 'love'] },
      { emoji: '\u{1F496}', name: 'sparkling heart', keywords: ['heart', 'sparkle'] },
      { emoji: '\u{1F497}', name: 'growing heart', keywords: ['heart', 'grow'] },
      { emoji: '\u{1F498}', name: 'heart with arrow', keywords: ['heart', 'cupid'] },
      { emoji: '\u{1F49D}', name: 'heart with ribbon', keywords: ['heart', 'gift'] },
      { emoji: '\u{1F49E}', name: 'revolving hearts', keywords: ['hearts', 'love'] },
      { emoji: '\u{1F49F}', name: 'heart decoration', keywords: ['heart'] },
      { emoji: '\u{1F4AF}', name: 'hundred points', keywords: ['100', 'perfect', 'score'] },
      { emoji: '\u{1F4A2}', name: 'anger symbol', keywords: ['angry'] },
      { emoji: '\u{1F4A3}', name: 'bomb', keywords: ['bomb'] },
      { emoji: '\u{1F4A4}', name: 'zzz', keywords: ['sleep', 'zzz'] },
      { emoji: '\u{1F4AC}', name: 'speech balloon', keywords: ['speech', 'chat', 'talk'] },
      { emoji: '\u{1F4AD}', name: 'thought balloon', keywords: ['thought', 'think'] },
      { emoji: '\u{1F4A1}', name: 'light bulb', keywords: ['idea', 'light'] },
      { emoji: '\u{1F4E7}', name: 'e-mail', keywords: ['email', 'mail'] },
      { emoji: '\u{1F4E8}', name: 'incoming envelope', keywords: ['email', 'mail'] },
      { emoji: '\u{1F4E9}', name: 'envelope with arrow', keywords: ['email', 'send'] },
      { emoji: '\u{270F}\u{FE0F}', name: 'pencil', keywords: ['pencil', 'write'] },
      { emoji: '\u{1F4DD}', name: 'memo', keywords: ['memo', 'note', 'write'] },
      { emoji: '\u{1F4C4}', name: 'page facing up', keywords: ['document', 'page'] },
      { emoji: '\u{1F4CB}', name: 'clipboard', keywords: ['clipboard'] },
      { emoji: '\u{1F4CA}', name: 'bar chart', keywords: ['chart', 'graph', 'stats'] },
      { emoji: '\u{1F4C8}', name: 'chart increasing', keywords: ['chart', 'up', 'growth'] },
      { emoji: '\u{1F4C9}', name: 'chart decreasing', keywords: ['chart', 'down'] },
      { emoji: '\u{1F4CE}', name: 'paperclip', keywords: ['paperclip', 'attach'] },
      { emoji: '\u{1F4CC}', name: 'pushpin', keywords: ['pin'] },
      { emoji: '\u{1F4D6}', name: 'open book', keywords: ['book', 'read'] },
      { emoji: '\u{1F4DA}', name: 'books', keywords: ['books', 'library'] },
      { emoji: '\u{1F517}', name: 'link', keywords: ['link', 'chain', 'url'] },
      { emoji: '\u{1F50D}', name: 'magnifying glass tilted left', keywords: ['search', 'find'] },
      { emoji: '\u{1F512}', name: 'locked', keywords: ['lock', 'secure'] },
      { emoji: '\u{1F513}', name: 'unlocked', keywords: ['unlock'] },
      { emoji: '\u{1F511}', name: 'key', keywords: ['key', 'password'] },
      { emoji: '\u{1F527}', name: 'wrench', keywords: ['wrench', 'tool'] },
      { emoji: '\u{1F528}', name: 'hammer', keywords: ['hammer', 'tool'] },
      { emoji: '\u{1F529}', name: 'nut and bolt', keywords: ['nut', 'bolt'] },
      { emoji: '\u{2699}\u{FE0F}', name: 'gear', keywords: ['gear', 'settings'] },
      { emoji: '\u{1F4BB}', name: 'laptop', keywords: ['computer', 'laptop'] },
      { emoji: '\u{1F4F1}', name: 'mobile phone', keywords: ['phone', 'mobile'] },
      { emoji: '\u{1F4F7}', name: 'camera', keywords: ['camera', 'photo'] },
      { emoji: '\u{1F4F9}', name: 'video camera', keywords: ['video', 'camera'] },
      { emoji: '\u{1F4FA}', name: 'television', keywords: ['tv', 'television'] },
      { emoji: '\u{1F50B}', name: 'battery', keywords: ['battery'] },
      { emoji: '\u{1F50C}', name: 'electric plug', keywords: ['plug', 'electric'] },
      { emoji: '\u{1F4B0}', name: 'money bag', keywords: ['money', 'rich'] },
      { emoji: '\u{1F4B5}', name: 'dollar banknote', keywords: ['money', 'dollar'] },
      { emoji: '\u{1F4B8}', name: 'money with wings', keywords: ['money', 'fly'] },
      { emoji: '\u{1F4B3}', name: 'credit card', keywords: ['credit card', 'pay'] },
      { emoji: '\u{1F48E}', name: 'gem stone', keywords: ['gem', 'diamond'] },
      { emoji: '\u{1F381}', name: 'wrapped gift', keywords: ['gift', 'present'] },
      { emoji: '\u{1F388}', name: 'balloon', keywords: ['balloon', 'party'] },
      { emoji: '\u{1F389}', name: 'party popper', keywords: ['party', 'celebrate'] },
      { emoji: '\u{1F38A}', name: 'confetti ball', keywords: ['confetti', 'party'] },
      { emoji: '\u{1F38E}', name: 'Japanese dolls', keywords: ['dolls', 'japanese'] },
      { emoji: '\u{1F3C6}', name: 'trophy', keywords: ['trophy', 'winner', 'award'] },
      { emoji: '\u{1F3C5}', name: 'sports medal', keywords: ['medal'] },
      { emoji: '\u{1F947}', name: 'first place medal', keywords: ['gold', 'medal', 'winner'] },
      { emoji: '\u{1F948}', name: 'second place medal', keywords: ['silver', 'medal'] },
      { emoji: '\u{1F949}', name: 'third place medal', keywords: ['bronze', 'medal'] },
    ],
  },
  {
    id: 'symbols',
    name: 'Symbols',
    icon: '\u{2764}\u{FE0F}',
    emojis: [
      { emoji: '\u{2705}', name: 'check mark button', keywords: ['check', 'yes', 'done'] },
      { emoji: '\u{274C}', name: 'cross mark', keywords: ['no', 'cross', 'wrong'] },
      { emoji: '\u{274E}', name: 'cross mark button', keywords: ['no'] },
      { emoji: '\u{2B55}', name: 'hollow red circle', keywords: ['circle', 'ok'] },
      { emoji: '\u{1F4A0}', name: 'diamond with a dot', keywords: ['diamond'] },
      { emoji: '\u{267B}\u{FE0F}', name: 'recycling symbol', keywords: ['recycle'] },
      { emoji: '\u{1F503}', name: 'clockwise arrows', keywords: ['reload', 'refresh'] },
      { emoji: '\u{1F504}', name: 'counter-clockwise arrows', keywords: ['reload', 'refresh'] },
      { emoji: '\u{2139}\u{FE0F}', name: 'information', keywords: ['info'] },
      { emoji: '\u{1F195}', name: 'new button', keywords: ['new'] },
      { emoji: '\u{1F197}', name: 'OK button', keywords: ['ok'] },
      { emoji: '\u{1F199}', name: 'UP! button', keywords: ['up'] },
      { emoji: '\u{1F192}', name: 'cool button', keywords: ['cool'] },
      { emoji: '\u{1F193}', name: 'free button', keywords: ['free'] },
      { emoji: '\u{0030}\u{FE0F}\u20E3', name: 'keycap 0', keywords: ['zero', '0'] },
      { emoji: '\u{0031}\u{FE0F}\u20E3', name: 'keycap 1', keywords: ['one', '1'] },
      { emoji: '\u{0032}\u{FE0F}\u20E3', name: 'keycap 2', keywords: ['two', '2'] },
      { emoji: '\u{0033}\u{FE0F}\u20E3', name: 'keycap 3', keywords: ['three', '3'] },
      { emoji: '\u{0034}\u{FE0F}\u20E3', name: 'keycap 4', keywords: ['four', '4'] },
      { emoji: '\u{0035}\u{FE0F}\u20E3', name: 'keycap 5', keywords: ['five', '5'] },
      { emoji: '\u{0036}\u{FE0F}\u20E3', name: 'keycap 6', keywords: ['six', '6'] },
      { emoji: '\u{0037}\u{FE0F}\u20E3', name: 'keycap 7', keywords: ['seven', '7'] },
      { emoji: '\u{0038}\u{FE0F}\u20E3', name: 'keycap 8', keywords: ['eight', '8'] },
      { emoji: '\u{0039}\u{FE0F}\u20E3', name: 'keycap 9', keywords: ['nine', '9'] },
      { emoji: '\u{1F51F}', name: 'keycap 10', keywords: ['ten', '10'] },
      { emoji: '\u{1F520}', name: 'input latin uppercase', keywords: ['abc', 'uppercase'] },
      { emoji: '\u{1F521}', name: 'input latin lowercase', keywords: ['abc', 'lowercase'] },
      { emoji: '\u{1F522}', name: 'input numbers', keywords: ['numbers', '123'] },
      { emoji: '\u{1F523}', name: 'input symbols', keywords: ['symbols'] },
      { emoji: '\u{1F524}', name: 'input latin letters', keywords: ['abc'] },
      { emoji: '\u{25B6}\u{FE0F}', name: 'play button', keywords: ['play'] },
      { emoji: '\u{23F8}\u{FE0F}', name: 'pause button', keywords: ['pause'] },
      { emoji: '\u{23F9}\u{FE0F}', name: 'stop button', keywords: ['stop'] },
      { emoji: '\u{23FA}\u{FE0F}', name: 'record button', keywords: ['record'] },
      { emoji: '\u{23ED}\u{FE0F}', name: 'next track button', keywords: ['next'] },
      { emoji: '\u{23EE}\u{FE0F}', name: 'last track button', keywords: ['previous'] },
      { emoji: '\u{1F500}', name: 'shuffle tracks button', keywords: ['shuffle'] },
      { emoji: '\u{1F501}', name: 'repeat button', keywords: ['repeat'] },
      { emoji: '\u{1F502}', name: 'repeat single button', keywords: ['repeat'] },
      { emoji: '\u{25C0}\u{FE0F}', name: 'reverse button', keywords: ['reverse'] },
      { emoji: '\u{1F53C}', name: 'upwards button', keywords: ['up'] },
      { emoji: '\u{1F53D}', name: 'downwards button', keywords: ['down'] },
      { emoji: '\u{2757}', name: 'red exclamation mark', keywords: ['exclamation', 'important'] },
      { emoji: '\u{2753}', name: 'red question mark', keywords: ['question'] },
      { emoji: '\u{2049}\u{FE0F}', name: 'exclamation question mark', keywords: ['exclamation', 'question'] },
      { emoji: '\u{203C}\u{FE0F}', name: 'double exclamation mark', keywords: ['exclamation'] },
      { emoji: '\u{1F51E}', name: 'no one under eighteen', keywords: ['18', 'adult'] },
      { emoji: '\u{1F4F3}', name: 'vibration mode', keywords: ['vibrate'] },
      { emoji: '\u{1F4F4}', name: 'mobile phone off', keywords: ['off', 'silent'] },
    ],
  },
  {
    id: 'flags',
    name: 'Flags',
    icon: '\u{1F3C1}',
    emojis: [
      { emoji: '\u{1F3C1}', name: 'chequered flag', keywords: ['race', 'finish'] },
      { emoji: '\u{1F6A9}', name: 'triangular flag', keywords: ['flag'] },
      { emoji: '\u{1F3F4}', name: 'black flag', keywords: ['flag'] },
      { emoji: '\u{1F3F3}\u{FE0F}', name: 'white flag', keywords: ['flag', 'surrender'] },
      { emoji: '\u{1F3F3}\u{FE0F}\u200D\u{1F308}', name: 'rainbow flag', keywords: ['pride', 'rainbow', 'lgbtq'] },
      { emoji: '\u{1F3F3}\u{FE0F}\u200D\u26A7\u{FE0F}', name: 'transgender flag', keywords: ['trans', 'pride'] },
      { emoji: '\u{1F1FA}\u{1F1F8}', name: 'flag: United States', keywords: ['us', 'usa', 'america'] },
      { emoji: '\u{1F1EC}\u{1F1E7}', name: 'flag: United Kingdom', keywords: ['uk', 'britain'] },
      { emoji: '\u{1F1E8}\u{1F1E6}', name: 'flag: Canada', keywords: ['canada'] },
      { emoji: '\u{1F1E6}\u{1F1FA}', name: 'flag: Australia', keywords: ['australia'] },
      { emoji: '\u{1F1E9}\u{1F1EA}', name: 'flag: Germany', keywords: ['germany'] },
      { emoji: '\u{1F1EB}\u{1F1F7}', name: 'flag: France', keywords: ['france'] },
      { emoji: '\u{1F1EE}\u{1F1F9}', name: 'flag: Italy', keywords: ['italy'] },
      { emoji: '\u{1F1EA}\u{1F1F8}', name: 'flag: Spain', keywords: ['spain'] },
      { emoji: '\u{1F1F5}\u{1F1F9}', name: 'flag: Portugal', keywords: ['portugal'] },
      { emoji: '\u{1F1E7}\u{1F1F7}', name: 'flag: Brazil', keywords: ['brazil'] },
      { emoji: '\u{1F1F2}\u{1F1FD}', name: 'flag: Mexico', keywords: ['mexico'] },
      { emoji: '\u{1F1EF}\u{1F1F5}', name: 'flag: Japan', keywords: ['japan'] },
      { emoji: '\u{1F1F0}\u{1F1F7}', name: 'flag: South Korea', keywords: ['korea'] },
      { emoji: '\u{1F1E8}\u{1F1F3}', name: 'flag: China', keywords: ['china'] },
      { emoji: '\u{1F1EE}\u{1F1F3}', name: 'flag: India', keywords: ['india'] },
      { emoji: '\u{1F1F7}\u{1F1FA}', name: 'flag: Russia', keywords: ['russia'] },
      { emoji: '\u{1F1F8}\u{1F1E6}', name: 'flag: Saudi Arabia', keywords: ['saudi'] },
      { emoji: '\u{1F1F9}\u{1F1F7}', name: 'flag: Turkey', keywords: ['turkey'] },
      { emoji: '\u{1F1F3}\u{1F1EC}', name: 'flag: Nigeria', keywords: ['nigeria'] },
      { emoji: '\u{1F1FF}\u{1F1E6}', name: 'flag: South Africa', keywords: ['south africa'] },
      { emoji: '\u{1F1F8}\u{1F1EA}', name: 'flag: Sweden', keywords: ['sweden'] },
      { emoji: '\u{1F1F3}\u{1F1F4}', name: 'flag: Norway', keywords: ['norway'] },
      { emoji: '\u{1F1E8}\u{1F1ED}', name: 'flag: Switzerland', keywords: ['switzerland'] },
      { emoji: '\u{1F1F3}\u{1F1FF}', name: 'flag: New Zealand', keywords: ['new zealand'] },
    ],
  },
];

// ============================================
// localStorage helpers for recently used emojis
// ============================================

const RECENT_EMOJIS_KEY = 'harbor-recent-emojis';
const MAX_RECENT = 32;

function getRecentEmojis(): string[] {
  try {
    const stored = localStorage.getItem(RECENT_EMOJIS_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch {
    // ignore parse errors
  }
  return [];
}

function addRecentEmoji(emoji: string): string[] {
  const recent = getRecentEmojis().filter((e) => e !== emoji);
  recent.unshift(emoji);
  const trimmed = recent.slice(0, MAX_RECENT);
  try {
    localStorage.setItem(RECENT_EMOJIS_KEY, JSON.stringify(trimmed));
  } catch {
    // ignore storage errors
  }
  return trimmed;
}

// ============================================
// Skin tone storage
// ============================================

const SKIN_TONE_KEY = 'harbor-emoji-skin-tone';

function getStoredSkinTone(): number {
  try {
    const stored = localStorage.getItem(SKIN_TONE_KEY);
    if (stored !== null) {
      const val = parseInt(stored, 10);
      if (val >= 0 && val < SKIN_TONES.length) return val;
    }
  } catch {
    // ignore
  }
  return 0;
}

function storeSkinTone(index: number) {
  try {
    localStorage.setItem(SKIN_TONE_KEY, String(index));
  } catch {
    // ignore
  }
}

// ============================================
// Apply skin tone modifier to an emoji
// ============================================

function applySkinTone(emoji: string, toneIndex: number): string {
  if (toneIndex === 0) return emoji; // default, no modifier
  if (!SKIN_TONE_ELIGIBLE.has(emoji)) return emoji;
  return emoji + SKIN_TONES[toneIndex].modifier;
}

// ============================================
// EmojiPicker Component
// ============================================

interface EmojiPickerProps {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

export function EmojiPicker({ onSelect, onClose }: EmojiPickerProps) {
  const [activeCategory, setActiveCategory] = useState('smileys');
  const [searchQuery, setSearchQuery] = useState('');
  const [recentEmojis, setRecentEmojis] = useState<string[]>(getRecentEmojis);
  const [skinToneIndex, setSkinToneIndex] = useState<number>(getStoredSkinTone);
  const [showSkinTones, setShowSkinTones] = useState(false);

  const pickerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const gridContainerRef = useRef<HTMLDivElement>(null);
  const categoryRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // Delay adding the listener to prevent the emoji button click from immediately closing
    const timeout = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);
    return () => {
      clearTimeout(timeout);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [onClose]);

  // Close on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  // Focus search on open
  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  // Search filtering
  const searchResults = useMemo(() => {
    if (!searchQuery.trim()) return null;
    const query = searchQuery.toLowerCase();
    const results: EmojiItem[] = [];
    for (const category of EMOJI_CATEGORIES) {
      for (const emoji of category.emojis) {
        if (
          emoji.name.toLowerCase().includes(query) ||
          emoji.keywords.some((k) => k.includes(query))
        ) {
          results.push(emoji);
        }
      }
    }
    return results;
  }, [searchQuery]);

  const handleEmojiClick = useCallback(
    (emoji: string) => {
      const withTone = applySkinTone(emoji, skinToneIndex);
      const newRecent = addRecentEmoji(withTone);
      setRecentEmojis(newRecent);
      onSelect(withTone);
    },
    [onSelect, skinToneIndex],
  );

  const handleSkinToneSelect = (index: number) => {
    setSkinToneIndex(index);
    storeSkinTone(index);
    setShowSkinTones(false);
  };

  // Scroll to category on tab click
  const scrollToCategory = (categoryId: string) => {
    setActiveCategory(categoryId);
    const el = categoryRefs.current.get(categoryId);
    if (el && gridContainerRef.current) {
      gridContainerRef.current.scrollTo({
        top: el.offsetTop - gridContainerRef.current.offsetTop,
        behavior: 'smooth',
      });
    }
  };

  // Track active category on scroll
  const handleScroll = useCallback(() => {
    if (searchQuery.trim()) return; // Don't track during search
    const container = gridContainerRef.current;
    if (!container) return;

    const containerTop = container.scrollTop + container.offsetTop;
    let closestCategory = 'smileys';
    let closestDistance = Infinity;

    categoryRefs.current.forEach((el, id) => {
      const distance = Math.abs(el.offsetTop - containerTop - container.offsetTop);
      if (distance < closestDistance) {
        closestDistance = distance;
        closestCategory = id;
      }
    });

    setActiveCategory(closestCategory);
  }, [searchQuery]);

  const renderEmojiGrid = (emojis: EmojiItem[], label?: string) => (
    <div>
      {label && (
        <p
          className="text-xs font-medium uppercase tracking-wider px-1 py-1.5 sticky top-0 z-10"
          style={{
            color: 'hsl(var(--harbor-text-tertiary))',
            background: 'hsl(var(--harbor-bg-elevated))',
          }}
        >
          {label}
        </p>
      )}
      <div className="grid grid-cols-8 gap-0.5">
        {emojis.map((item, i) => (
          <button
            key={`${item.emoji}-${i}`}
            onClick={() => handleEmojiClick(item.emoji)}
            className="w-9 h-9 flex items-center justify-center rounded-lg text-xl transition-colors hover:bg-white/10 active:scale-90"
            title={item.name}
            type="button"
          >
            {applySkinTone(item.emoji, skinToneIndex)}
          </button>
        ))}
      </div>
    </div>
  );

  return (
    <div
      ref={pickerRef}
      className="absolute bottom-full mb-2 right-0 w-[340px] rounded-xl shadow-xl z-50 overflow-hidden animate-fade-in-scale"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      {/* Search bar & skin tone */}
      <div
        className="px-3 pt-3 pb-2 flex items-center gap-2"
        style={{ borderBottom: '1px solid hsl(var(--harbor-border-subtle))' }}
      >
        <div className="flex-1 relative">
          <SearchIcon
            className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          />
          <input
            ref={searchInputRef}
            type="text"
            placeholder="Search emojis..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 rounded-lg text-xs"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              color: 'hsl(var(--harbor-text-primary))',
            }}
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery('')}
              className="absolute right-2 top-1/2 -translate-y-1/2"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              type="button"
            >
              <XIcon className="w-3 h-3" />
            </button>
          )}
        </div>
        {/* Skin tone selector */}
        <div className="relative">
          <button
            onClick={() => setShowSkinTones(!showSkinTones)}
            className="w-8 h-8 flex items-center justify-center rounded-lg text-lg transition-colors hover:bg-white/10"
            title="Skin tone"
            type="button"
            style={{
              background: showSkinTones ? 'hsl(var(--harbor-surface-2))' : 'transparent',
            }}
          >
            {SKIN_TONES[skinToneIndex].preview}
          </button>
          {showSkinTones && (
            <div
              className="absolute right-0 top-full mt-1 rounded-lg shadow-lg z-50 p-1 flex gap-0.5"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              {SKIN_TONES.map((tone, i) => (
                <button
                  key={i}
                  onClick={() => handleSkinToneSelect(i)}
                  className="w-8 h-8 flex items-center justify-center rounded-md text-lg transition-colors hover:bg-white/10"
                  title={tone.label}
                  type="button"
                  style={{
                    background:
                      i === skinToneIndex ? 'hsl(var(--harbor-primary) / 0.2)' : 'transparent',
                  }}
                >
                  {tone.preview}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Category tabs (hidden during search) */}
      {!searchQuery.trim() && (
        <div
          className="flex px-1 py-1 gap-0.5 overflow-x-auto"
          style={{ borderBottom: '1px solid hsl(var(--harbor-border-subtle))' }}
        >
          {recentEmojis.length > 0 && (
            <button
              onClick={() => scrollToCategory('recent')}
              className="flex-shrink-0 w-8 h-8 flex items-center justify-center rounded-lg text-sm transition-colors"
              title="Recently Used"
              type="button"
              style={{
                background:
                  activeCategory === 'recent'
                    ? 'hsl(var(--harbor-primary) / 0.15)'
                    : 'transparent',
                color:
                  activeCategory === 'recent'
                    ? 'hsl(var(--harbor-primary))'
                    : 'hsl(var(--harbor-text-tertiary))',
              }}
            >
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </button>
          )}
          {EMOJI_CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              onClick={() => scrollToCategory(cat.id)}
              className="flex-shrink-0 w-8 h-8 flex items-center justify-center rounded-lg text-sm transition-colors"
              title={cat.name}
              type="button"
              style={{
                background:
                  activeCategory === cat.id
                    ? 'hsl(var(--harbor-primary) / 0.15)'
                    : 'transparent',
                color:
                  activeCategory === cat.id
                    ? 'hsl(var(--harbor-primary))'
                    : 'hsl(var(--harbor-text-tertiary))',
              }}
            >
              {cat.icon}
            </button>
          ))}
        </div>
      )}

      {/* Emoji grid */}
      <div
        ref={gridContainerRef}
        className="overflow-y-auto px-2 py-1"
        style={{ height: '280px' }}
        onScroll={handleScroll}
      >
        {searchQuery.trim() ? (
          // Search results
          searchResults && searchResults.length > 0 ? (
            renderEmojiGrid(
              searchResults,
              `${searchResults.length} result${searchResults.length !== 1 ? 's' : ''}`,
            )
          ) : (
            <div className="flex items-center justify-center h-full">
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                No emojis found
              </p>
            </div>
          )
        ) : (
          // Browsing mode with categories
          <>
            {recentEmojis.length > 0 && (
              <div
                ref={(el) => {
                  if (el) categoryRefs.current.set('recent', el);
                }}
              >
                {renderEmojiGrid(
                  recentEmojis.map((emoji) => ({
                    emoji,
                    name: 'recently used',
                    keywords: ['recent'],
                  })),
                  'Recently Used',
                )}
              </div>
            )}
            {EMOJI_CATEGORIES.map((category) => (
              <div
                key={category.id}
                ref={(el) => {
                  if (el) categoryRefs.current.set(category.id, el);
                }}
              >
                {renderEmojiGrid(category.emojis, category.name)}
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
