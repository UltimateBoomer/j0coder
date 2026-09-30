import java.lang.reflect.*;
import java.math.BigInteger;
import java.util.*;

/** Run via tests/jvm_adapter_fixtures.py with a JDK and Jackson classpath. */
public class JvmAdapterFixtures {
    static class Node { public String __judgeId; public Integer value; public Node next; }
    static class Graph { public List<Node> roots=new ArrayList<>(); public List<Node> nodes=new ArrayList<>(); }
    static class Overloaded {
        public Overloaded(String x) {}
        public Overloaded(int x) {}
        public int echo(String x) { return 0; }
        public int echo(int x) { return x; }
    }
    static void require(boolean condition) { if (!condition) throw new AssertionError(); }
    static void rejects(Object value, String type) throws Exception {
        try { JudgeMain.encode(value, JudgeMain.JSON.readTree("\""+type+"\"")); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("accepted invalid " + type + " output " + value);
    }
    public static void main(String[] ignored) throws Exception {
        require(JudgeMain.encode(Integer.MIN_VALUE,JudgeMain.JSON.readTree("\"int\"")).intValue()==Integer.MIN_VALUE);
        require(JudgeMain.encode(Long.MAX_VALUE,JudgeMain.JSON.readTree("\"int64\"")).longValue()==Long.MAX_VALUE);
        rejects(4294967297L,"int"); rejects(-2147483649L,"int");
        rejects(1.5,"int"); rejects(1.0,"int64");
        rejects(BigInteger.valueOf(Long.MAX_VALUE).add(BigInteger.ONE),"int64");
        var params=JudgeMain.JSON.readTree("[{\"name\":\"x\",\"ty\":\"int\"}]");
        Method method=(Method)JudgeMain.find(Overloaded.class,"echo",params,false);
        Constructor<?> constructor=(Constructor<?>)JudgeMain.find(Overloaded.class,"<init>",params,true);
        require(method.invoke(constructor.newInstance(7),7).equals(7));
        var values=JudgeMain.F.arrayNode();
        for(int i=0;i<6000;i++) values.add(i);
        var arrayType=JudgeMain.JSON.readTree("{\"array\":\"int\"}");
        var arrayParams=JudgeMain.JSON.readTree("[{\"name\":\"a\",\"ty\":{\"array\":\"int\"}},{\"name\":\"b\",\"ty\":{\"array\":\"int\"}}]");
        var decoded=JudgeMain.args(JudgeMain.F.arrayNode().add(values).add(values),arrayParams);
        require(JudgeMain.encodeValue(decoded[0],arrayType).equals(values));
        require(JudgeMain.encodeValue(decoded[1],arrayType).equals(values));
        values.removeAll(); for(int i=0;i<10000;i++) values.add(i);
        try { JudgeMain.decodeValue(values,arrayType); throw new AssertionError("oversized input accepted"); }
        catch(IllegalArgumentException expected) {}
        require(JudgeMain.graphNodeName("Graph").equals("GraphNode"));
        JudgeMain.definitions.put("GraphNode",JudgeMain.F.objectNode());
        require(JudgeMain.graphNodeName("Graph").equals("__JudgeGraphNode_Graph"));
        JudgeMain.definitions.remove("GraphNode");
        JudgeMain.definitions.put("Graph",JudgeMain.JSON.readTree("{\"name\":\"Graph\",\"codec\":\"object_graph\",\"fields\":[{\"name\":\"value\",\"ty\":\"int\"},{\"name\":\"next\",\"ty\":{\"nullable\":{\"named\":\"Graph\"}}}]}"));
        Graph graph=new Graph(); Node added=new Node();added.value=7;
        Node original=new Node();original.value=1;original.__judgeId="new1";
        graph.nodes.add(added);graph.nodes.add(original);
        var out=JudgeMain.encodeValue(graph,JudgeMain.JSON.readTree("{\"named\":\"Graph\"}"));
        require(out.path("nodes").size()==2);
        require(out.path("nodes").get(0).path("id").asText().equals("new1"));
        require(out.path("nodes").get(1).path("id").asText().equals("new2"));
        require(out.path("nodes").get(1).path("value").asInt()==7);
        System.out.println("JVM adapter regressions passed");
    }
}
