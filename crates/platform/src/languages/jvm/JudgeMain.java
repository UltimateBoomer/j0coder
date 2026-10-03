import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Fixed adapter; user code is loaded from the per-submission JAR. */
public final class JudgeMain {
    static final ObjectMapper JSON = new ObjectMapper();
    static final JsonNodeFactory F = JsonNodeFactory.instance;
    static JsonNode schema;
    static Map<String, JsonNode> definitions = new HashMap<>();
    static int count;

    static void tick() {
        if (++count > 10000) throw new IllegalArgumentException("structured value limit exceeded");
    }
    static Object decodeValue(JsonNode v, JsonNode t) throws Exception {
        count = 0;
        return decode(v, t);
    }
    static JsonNode encodeValue(Object v, JsonNode t) throws Exception {
        count = 0;
        return encode(v, t);
    }
    static String graphNodeName(String name) {
        String candidate = name + "Node";
        boolean interfaceCollision = schema != null && schema.path("interface").path("kind").asText().equals("data_structure")
            && schema.path("interface").path("name").asText().equals(candidate);
        return definitions.containsKey(candidate) || interfaceCollision ? "__JudgeGraphNode_" + name : candidate;
    }
    static String kind(JsonNode type) {
        if (type.isTextual()) return type.asText();
        return type.fieldNames().next();
    }
    static Field field(Class<?> owner, String name) throws Exception {
        Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }
    static Object fresh(String name) throws Exception {
        Constructor<?> c = Class.forName(name).getDeclaredConstructor();
        c.setAccessible(true);
        return c.newInstance();
    }
    static JsonNode def(String name) {
        JsonNode d = definitions.get(name);
        if (d == null) throw new IllegalArgumentException("unknown model " + name);
        return d;
    }
    static JsonNode valueType(JsonNode d) {
        for (JsonNode f : d.path("fields")) if (f.path("name").asText().matches("value|val")) return f.path("ty");
        throw new IllegalArgumentException("missing value field");
    }
    static String valueField(JsonNode d) {
        for (JsonNode f : d.path("fields")) if (f.path("name").asText().matches("value|val")) return f.path("name").asText();
        throw new IllegalArgumentException("missing value field");
    }
    static Object decode(JsonNode v, JsonNode t) throws Exception {
        tick();
        String k = kind(t);
        if (k.equals("nullable")) return v.isNull() ? null : decode(v, t.get(k));
        if (v.isNull()) {
            if (k.equals("named") && def(t.get(k).asText()).path("codec").asText("record").equals("nary_tree")) return null;
            throw new IllegalArgumentException("null for " + k);
        }
        switch (k) {
            case "int": if (!v.isIntegralNumber() || !v.canConvertToInt()) throw new IllegalArgumentException("int32 required"); return v.intValue();
            case "int64": if (!v.isIntegralNumber() || !v.canConvertToLong()) throw new IllegalArgumentException("int64 required"); return v.longValue();
            case "float": if (!v.isNumber() || !Double.isFinite(v.doubleValue())) throw new IllegalArgumentException("finite number required"); return v.doubleValue();
            case "bool": if (!v.isBoolean()) throw new IllegalArgumentException("bool required"); return v.booleanValue();
            case "string": if (!v.isTextual()) throw new IllegalArgumentException("string required"); return v.textValue();
            case "array": {
                if (!v.isArray()) throw new IllegalArgumentException("array required");
                List<Object> out = new ArrayList<>();
                for (JsonNode x : v) out.add(decode(x, t.get(k)));
                return out;
            }
            case "named": return decodeNamed(v, t.get(k).asText());
            default: throw new IllegalArgumentException("unsupported type " + k);
        }
    }
    static Object decodeNamed(JsonNode v, String name) throws Exception {
        JsonNode d = def(name);
        String codec = d.path("codec").asText("record");
        switch (codec) {
            case "record": {
                Object o = fresh(name);
                for (JsonNode f : d.path("fields")) field(o.getClass(), f.path("name").asText()).set(o, decode(v.get(f.path("name").asText()), f.path("ty")));
                return o;
            }
            case "singly_linked_list": {
                if (!v.isArray()) throw new IllegalArgumentException("list array required");
                Object head = null, previous = null;
                for (JsonNode x : v) {
                    Object node = fresh(name);
                    field(node.getClass(), valueField(d)).set(node, decode(x, valueType(d)));
                    if (previous == null) head = node; else field(previous.getClass(), "next").set(previous, node);
                    previous = node;
                }
                return head;
            }
            case "binary_tree": {
                if (!v.isArray()) throw new IllegalArgumentException("tree array required");
                if (v.isEmpty()) return null;
                List<Object> nodes = new ArrayList<>();
                for (JsonNode x : v) {
                    Object node = x.isNull() ? null : fresh(name);
                    if (node != null) field(node.getClass(), valueField(d)).set(node, decode(x, valueType(d)));
                    nodes.add(node);
                }
                int next = 1;
                for (Object node : nodes) if (node != null) {
                    if (next < nodes.size()) field(node.getClass(), "left").set(node, nodes.get(next++));
                    if (next < nodes.size()) field(node.getClass(), "right").set(node, nodes.get(next++));
                }
                return nodes.get(0);
            }
            case "nary_tree": return decodeNary(v, name, d);
            case "object_graph": return decodeGraph(v, name, d);
            default: throw new IllegalArgumentException("unsupported codec " + codec);
        }
    }
    static Object decodeNary(JsonNode v, String name, JsonNode d) throws Exception {
        if (v.isNull()) return null;
        Object node = fresh(name);
        field(node.getClass(), valueField(d)).set(node, decode(v.path("value"), valueType(d)));
        List<Object> children = new ArrayList<>();
        for (JsonNode child : v.path("children")) children.add(decodeNary(child, name, d));
        field(node.getClass(), "children").set(node, children);
        return node;
    }
    static Object decodeGraph(JsonNode v, String name, JsonNode d) throws Exception {
        Object graph = fresh(name);
        Map<String,Object> byId = new LinkedHashMap<>();
        for (JsonNode item : v.path("nodes")) {
            Object node = fresh(graphNodeName(name));
            String id = item.path("id").asText();
            field(node.getClass(), "__judgeId").set(node, id);
            if (byId.put(id,node) != null) throw new IllegalArgumentException("duplicate graph ID");
        }
        for (JsonNode item : v.path("nodes")) {
            Object node = byId.get(item.path("id").asText());
            for (JsonNode f : d.path("fields")) {
                String fn = f.path("name").asText();
                JsonNode ft = f.path("ty"), x = item.path(fn);
                Object value = isGraphRef(ft,name) ? (x.isNull() ? null : byId.get(x.asText())) : decode(x,ft);
                field(node.getClass(),fn).set(node,value);
            }
        }
        List<Object> roots = new ArrayList<>();
        for (JsonNode root : v.path("roots")) roots.add(root.isNull() ? null : byId.get(root.asText()));
        field(graph.getClass(),"roots").set(graph,roots);
        field(graph.getClass(),"nodes").set(graph,new ArrayList<>(byId.values()));
        return graph;
    }
    static boolean isGraphRef(JsonNode t, String name) {
        return (kind(t).equals("named") && t.get("named").asText().equals(name)) ||
            (kind(t).equals("nullable") && isGraphRef(t.get("nullable"),name));
    }
    static JsonNode encode(Object v, JsonNode t) throws Exception {
        tick();
        String k = kind(t);
        if (k.equals("nullable")) return v == null ? F.nullNode() : encode(v,t.get(k));
        if (k.equals("void")) return F.nullNode();
        if (v == null) {
            if (k.equals("named")) { String codec=def(t.get(k).asText()).path("codec").asText("record"); if(codec.equals("singly_linked_list")||codec.equals("binary_tree"))return F.arrayNode(); }
            return F.nullNode();
        }
        switch (k) {
            case "int": {
                JsonNode n = JSON.valueToTree(v);
                if (!n.isIntegralNumber() || !n.canConvertToInt()) throw new IllegalArgumentException("int32 output required");
                return F.numberNode(n.intValue());
            }
            case "int64": {
                JsonNode n = JSON.valueToTree(v);
                if (!n.isIntegralNumber() || !n.canConvertToLong()) throw new IllegalArgumentException("int64 output required");
                return F.numberNode(n.longValue());
            }
            case "float": {
                double n=((Number)v).doubleValue();
                if (!Double.isFinite(n)) throw new IllegalArgumentException("nonfinite output");
                return F.numberNode(n);
            }
            case "bool": return F.booleanNode((Boolean)v);
            case "string": return F.textNode((String)v);
            case "array": {
                ArrayNode out = F.arrayNode();
                for (Object x : (Iterable<?>)v) out.add(encode(x,t.get(k)));
                return out;
            }
            case "named": return encodeNamed(v,t.get(k).asText());
            default: throw new IllegalArgumentException("unsupported type " + k);
        }
    }
    static JsonNode encodeNamed(Object v, String name) throws Exception {
        JsonNode d=def(name);
        switch (d.path("codec").asText("record")) {
            case "record": {
                ObjectNode o=F.objectNode();
                for (JsonNode f:d.path("fields")) { String fn=f.path("name").asText(); o.set(fn,encode(field(v.getClass(),fn).get(v),f.path("ty"))); }
                return o;
            }
            case "singly_linked_list": {
                ArrayNode a=F.arrayNode(); Set<Object> seen=Collections.newSetFromMap(new IdentityHashMap<>());
                while(v!=null) { if(!seen.add(v))throw new IllegalArgumentException("list cycle"); a.add(encode(field(v.getClass(),valueField(d)).get(v),valueType(d))); v=field(v.getClass(),"next").get(v); }
                return a;
            }
            case "binary_tree": {
                ArrayNode a=F.arrayNode(); Queue<Object> q=new ArrayDeque<>(); q.add(v); Set<Object> seen=Collections.newSetFromMap(new IdentityHashMap<>());
                while(!q.isEmpty()) { Object node=q.remove(); if(node==SENTINEL){a.addNull();continue;} if(!seen.add(node))throw new IllegalArgumentException("tree cycle"); a.add(encode(field(node.getClass(),valueField(d)).get(node),valueType(d))); Object left=field(node.getClass(),"left").get(node),right=field(node.getClass(),"right").get(node); q.add(left==null?SENTINEL:left);q.add(right==null?SENTINEL:right); }
                while(a.size()>0&&a.get(a.size()-1).isNull())a.remove(a.size()-1);
                return a;
            }
            case "nary_tree": return encodeNary(v,d,new IdentityHashMap<>());
            case "object_graph": return encodeGraph(v,name,d);
            default: throw new IllegalArgumentException("unsupported codec");
        }
    }
    static final Object SENTINEL=new Object();
    static JsonNode encodeNary(Object v, JsonNode d, IdentityHashMap<Object,Boolean> seen) throws Exception {
        if(v==null)return F.nullNode();
        if(seen.put(v,true)!=null)throw new IllegalArgumentException("tree cycle");
        ObjectNode o=F.objectNode(); o.set("value",encode(field(v.getClass(),valueField(d)).get(v),valueType(d)));
        ArrayNode children=F.arrayNode();
        for(Object child:(Iterable<?>)field(v.getClass(),"children").get(v))children.add(encodeNary(child,d,seen));
        o.set("children",children);return o;
    }
    static JsonNode encodeGraph(Object graph,String name,JsonNode d) throws Exception {
        List<?> roots=(List<?>)field(graph.getClass(),"roots").get(graph);
        List<?> original=(List<?>)field(graph.getClass(),"nodes").get(graph);
        IdentityHashMap<Object,String> ids=new IdentityHashMap<>(); List<Object> order=new ArrayList<>(); Set<String> used=new HashSet<>();
        for(Object node:original)if(node!=null&&!ids.containsKey(node)){String id=(String)field(node.getClass(),"__judgeId").get(node);if(id!=null&&used.add(id)){ids.put(node,id);order.add(node);}}
        int[] next={1};
        for(Object node:original) graphId(node,ids,order,used,next);
        for(Object node:roots) graphId(node,ids,order,used,next);
        ArrayNode nodes=F.arrayNode();
        for(int i=0;i<10000&&i<order.size();i++){
            Object node=order.get(i);ObjectNode item=F.objectNode();item.put("id",ids.get(node));
            for(JsonNode f:d.path("fields")){String fn=f.path("name").asText();JsonNode ft=f.path("ty");Object val=field(node.getClass(),fn).get(node);
                if(isGraphRef(ft,name)){String id=graphId(val,ids,order,used,next);item.set(fn,id==null?F.nullNode():F.textNode(id));}
                else item.set(fn,encode(val,ft));
            }
            nodes.add(item);
        }
        if(nodes.size()!=ids.size())throw new IllegalArgumentException("graph too large");
        ArrayNode rootIds=F.arrayNode();for(Object node:roots){String id=graphId(node,ids,order,used,next);rootIds.add(id==null?F.nullNode():F.textNode(id));}
        ObjectNode out=F.objectNode();out.set("roots",rootIds);out.set("nodes",nodes);return out;
    }
    static String graphId(Object node,IdentityHashMap<Object,String> ids,List<Object> order,Set<String> used,int[] next){
        if(node==null)return null;String id=ids.get(node);if(id!=null)return id;
        do{id="new"+next[0]++;}while(used.contains(id));used.add(id);ids.put(node,id);order.add(node);return id;
    }
    static Class<?> parameterClass(JsonNode t) throws Exception {
        String k = kind(t);
        switch (k) {
            case "nullable": return parameterClass(t.get(k));
            case "int": return Integer.class;
            case "int64": return Long.class;
            case "float": return Double.class;
            case "bool": return Boolean.class;
            case "string": return String.class;
            case "array": return List.class;
            case "named": return Class.forName(t.get(k).asText());
            default: throw new IllegalArgumentException("unsupported parameter type " + k);
        }
    }
    static Class<?> boxed(Class<?> c) {
        if (c == int.class) return Integer.class;
        if (c == long.class) return Long.class;
        if (c == double.class) return Double.class;
        if (c == boolean.class) return Boolean.class;
        return c;
    }
    static Executable find(Class<?> c,String name,JsonNode params,boolean constructor) throws Exception {
        Class<?>[] expected = new Class<?>[params.size()];
        for (int i=0;i<expected.length;i++) expected[i]=parameterClass(params.get(i).path("ty"));
        Executable selected = null;
        Executable[] candidates = constructor ? c.getDeclaredConstructors() : c.getDeclaredMethods();
        for (Executable x : candidates) {
            if (!constructor && (!x.getName().equals(name) || ((Method)x).isBridge() || x.isSynthetic())) continue;
            Class<?>[] actual=x.getParameterTypes();
            if (actual.length != expected.length) continue;
            boolean matches=true;
            for (int i=0;i<actual.length;i++) if (boxed(actual[i]) != expected[i]) { matches=false;break; }
            if (!matches) continue;
            if (selected != null) throw new IllegalArgumentException("ambiguous signature " + name);
            selected=x;
        }
        if (selected == null) throw new NoSuchMethodException(name+"/"+params.size());
        selected.setAccessible(true);
        return selected;
    }
    static Object[] args(JsonNode values,JsonNode params) throws Exception {
        Object[] a=new Object[params.size()];
        if(!values.isArray()||values.size()!=a.length)throw new IllegalArgumentException("argument count");
        for(int i=0;i<a.length;i++)a[i]=decodeValue(values.get(i),params.get(i).path("ty"));
        return a;
    }
    public static void main(String[] ignored) throws Exception {
        try(InputStream in=JudgeMain.class.getClassLoader().getResourceAsStream("judge-schema.json")){
            if(in==null)throw new IllegalStateException("missing schema");schema=JSON.readTree(in);
        }
        for(JsonNode d:schema.path("type_definitions"))definitions.put(d.path("name").asText(),d);
        JsonNode input=JSON.readTree(System.in), iface=schema.path("interface");
        JsonNode result;
        if(iface.path("kind").asText().equals("function")){
            Class<?> c=Class.forName(schema.path("language").asText().equals("kotlin")?"SolutionKt":"Solution");
            Method m=(Method)find(c,iface.path("name").asText(),iface.path("params"),false);
            Object target=Modifier.isStatic(m.getModifiers())?null:fresh("Solution");
            result=encodeValue(m.invoke(target,args(input,iface.path("params"))),iface.path("returns"));
        }else{
            Class<?> c=Class.forName(iface.path("name").asText());JsonNode ctor=iface.path("constructor").path("params");
            Constructor<?> cons=(Constructor<?>)find(c,"<init>",ctor,true);
            Object target=cons.newInstance(args(input.path("constructor_args"),ctor));
            ArrayNode results=F.arrayNode();int index=0;
            for(JsonNode op:input.path("operations")){
                try{JsonNode method=null;for(JsonNode x:iface.path("methods"))if(x.path("name").asText().equals(op.path("method").asText()))method=x;
                    if(method==null)throw new IllegalArgumentException("undeclared method");
                    Method m=(Method)find(c,method.path("name").asText(),method.path("params"),false);
                    results.add(encodeValue(m.invoke(target,args(op.path("args"),method.path("params"))),method.path("returns")));
                }catch(Exception e){throw new IllegalStateException("operation "+index+" failed",e);}index++;
            }
            result=results;
        }
        try(Writer out=new FileWriter("/work/result")){JSON.writeValue(out,result);}
    }
}
