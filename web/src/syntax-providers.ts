import type {Language} from './languages';
// C++20 keywords and alternative operator spellings.
const cppKeywords = [
 'alignas','alignof','and','and_eq','asm','auto','bitand','bitor','bool','break',
 'case','catch','char','char8_t','char16_t','char32_t','class','compl','concept',
 'const','const_cast','consteval','constexpr','constinit','continue','co_await',
 'co_return','co_yield','decltype','default','delete','do','double','dynamic_cast',
 'else','enum','explicit','export','extern','false','float','for','friend','goto',
 'if','import','inline','int','long','module','mutable','namespace','new',
 'noexcept','not','not_eq','nullptr','operator','or','or_eq','private',
 'protected','public','register',
 'reinterpret_cast','requires','return','short','signed','sizeof','static',
 'static_assert','static_cast','struct','switch','template','this','thread_local',
 'throw','true','try','typedef','typeid','typename','union','unsigned','using',
 'virtual','void','volatile','wchar_t','while','xor','xor_eq'
];
export const syntaxProviders={
 cpp:{keywords:cppKeywords},
 python:{keywords:['class','def','return','if','else','elif','for','while','in','not','and','or','True','False','None','pass','import','from']},
 java:{keywords:['class','interface','public','private','static','void','int','long','double','boolean','new','return','if','else','for','while','null','true','false','import']},
 kotlin:{keywords:['class','fun','val','var','return','if','else','for','while','when','null','true','false','import','object']}
} satisfies Record<Language,{keywords:string[]}>;
