// Compile-time regressions for Serde defaults in generated request contracts.
import type {components} from './api.generated';
const definition:components['schemas']['TypeDefinitionInput']={name:'Node'};
const constructor:components['schemas']['ConstructorInput']={};
const parameter:components['schemas']['ParameterInput']={name:'value',ty:'int',constraints:null};
const problem:components['schemas']['ProblemInput']={schema:3,title:'Example',statement:'Example',difficulty:'easy',tags:[],interface:{kind:'function',name:'solve',returns:'int'},tests:[{args:[],expected:1}]};
void [definition,constructor,parameter,problem];
