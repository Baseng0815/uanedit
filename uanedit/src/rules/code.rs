use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

/// Whether a finding is a violation of the specification or a deviation it tolerates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// The specification says "shall"; an edit operation refuses to introduce one.
    Error,
    /// The specification permits it, but it is very likely a mistake.
    Warning,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Defines the whole rule registry in one place: the code, its identifier, its default severity,
/// its one-line message and the explanation that cites the clause it encodes.
macro_rules! registry {
    ($($variant:ident = $id:literal, $severity:ident, $summary:literal, $explanation:literal;)*) => {
        /// A rule of the engine, named by a stable `UA0000` identifier.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum DiagnosticCode {
            $($variant,)*
        }

        impl DiagnosticCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }

            /// The severity the rule reports unless a finding states a narrower one.
            pub fn severity(self) -> Severity {
                match self {
                    $(Self::$variant => Severity::$severity,)*
                }
            }

            pub fn summary(self) -> &'static str {
                match self {
                    $(Self::$variant => $summary,)*
                }
            }

            /// The `rustc --explain` text: what the rule means and which clause requires it.
            pub fn explanation(self) -> &'static str {
                match self {
                    $(Self::$variant => $explanation,)*
                }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                match id {
                    $($id => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

registry! {
    DanglingReferenceTarget = "UA0101", Warning,
        "reference target is in no loaded nodeset",
        "A reference names a NodeId that no loaded nodeset defines. OPC 10000-3 §4.4.4 does not \
         require the TargetNode to exist, so this is not a violation, but in a file under edit it \
         is almost always a missing dependency or a typo. Load the nodeset that defines the \
         target, or retarget the reference.";
    DuplicateNodeId = "UA0102", Error,
        "NodeId is defined by more than one loaded nodeset",
        "OPC 10000-3 §5.2.2 identifies a node unambiguously by its NodeId, so one NodeId cannot \
         stand for two nodes. Two loaded nodesets define a node with the same NodeId in the same \
         namespace; one of them has to give the node a different identifier.";
    SiblingBrowseNameCollision = "UA0103", Error,
        "two children share a BrowseName under a type or an instance declaration",
        "OPC 10000-3 §5.5.2 and §5.6.5 require the targets of forward hierarchical references of \
         an ObjectType or VariableType to have unique BrowseNames, and §6.2.6 extends that to \
         every instance declaration, so an instance declaration can be identified by its \
         BrowsePath. §5.3.3 and §5.8.3 state no such rule for ReferenceType or DataType nodes, \
         and a plain instance may hold children with duplicate BrowseNames (§6.4.2); neither is \
         reported.";
    UnknownNamespaceIndex = "UA0104", Error,
        "namespace index has no entry in the nodeset's namespace table",
        "A NodeId or BrowseName uses a namespace index the file's NamespaceUris table does not \
         declare, so the naming authority it refers to (OPC 10000-3 §8.2.2) cannot be resolved. \
         Add the namespace URI to the table, or restate the value with a declared index.";
    ReferenceTypeNotAReferenceType = "UA0105", Error,
        "the reference names a node that is not a ReferenceType",
        "Every reference is of a type defined by a ReferenceType node (OPC 10000-3 §5.3.1), and \
         OPC 10000-3 §7.2 requires every ReferenceType to be a subtype of References. The node \
         this reference names is of another NodeClass.";
    UnknownAlias = "UA0106", Error,
        "the reference names an alias the nodeset does not define",
        "A reference in a UANodeSet file may name a NodeId through the file's Aliases table (OPC \
         10000-6 Annex F). This one names an alias with no entry, so the reference resolves to \
         nothing and is left out of the address space. Add the alias, or spell the NodeId out.";
    DuplicateReference = "UA0107", Error,
        "the node states the same reference more than once",
        "OPC 10000-3 §4.4.4 identifies a reference by its SourceNode, ReferenceType and \
         TargetNode, so a node can reference another node with the same ReferenceType only once. \
         This node's own References element states one of them twice. Stating the same reference \
         once on each end is a different thing and not reported: OPC 10000-6 §F.4 only requires \
         one direction to be in the file and has the reader add the other, so both ends spelling \
         it out is redundant rather than a second reference.";
    NullNodeId = "UA0108", Error,
        "the node has the null NodeId",
        "OPC 10000-3 §8.2.4 states that a node in the address space shall not have a null NodeId, \
         which is namespace index 0 with the numeric identifier 0. Give the node an identifier in \
         the nodeset's own namespace.";
    ParentNodeIdMismatch = "UA0109", Warning,
        "ParentNodeId names a node that is not a hierarchical parent",
        "OPC 10000-6 §F.7 states that ParentNodeId shall reference a UANode which is the source \
         of a HierarchicalReference to the node, and that the information does not appear in the \
         address space. Because it is design-tool metadata rather than address-space content this \
         is reported as a warning, but a node reached only by a non-hierarchical reference is \
         outside what §F.7 allows the field to name.";
    DuplicatePropertyBrowseName = "UA0110", Error,
        "two Properties of this node share a BrowseName",
        "OPC 10000-3 §5.6.3 states that the BrowseName of a Property is always unique in the \
         context of a node, and that it is not permitted for a node to refer to two Variables \
         using HasProperty references having the same BrowseName. This holds for every node, not \
         only for types and instance declarations.";

    InvalidSourceNodeClass = "UA0201", Error,
        "the reference type does not allow this NodeClass as its source",
        "OPC 10000-3 §7 constrains the NodeClass at each end of every standard ReferenceType, and \
         §5.3.3.3 makes those constraints inherit to subtypes, where they may only be narrowed \
         further. The source of this reference is of a NodeClass the type does not allow.";
    InvalidTargetNodeClass = "UA0202", Error,
        "the reference type does not allow this NodeClass as its target",
        "OPC 10000-3 §7 constrains the NodeClass at each end of every standard ReferenceType, and \
         §5.3.3.3 makes those constraints inherit to subtypes. The target of this reference is of \
         a NodeClass the type does not allow.";
    HasSubtypeAcrossNodeClasses = "UA0203", Error,
        "HasSubtype relates nodes of different NodeClasses",
        "OPC 10000-3 §7.10 requires the source of a HasSubtype reference to be an ObjectType, a \
         VariableType, a DataType or a ReferenceType, and the target to be of the same NodeClass. \
         §6.3.1 restates it: subtyping can only occur between nodes of the same NodeClass.";
    MultipleSupertypes = "UA0205", Error,
        "the type has more than one supertype",
        "OPC 10000-3 §5.3.3.3 states that a ReferenceType shall have exactly one supertype and \
         that the hierarchy does not support multiple inheritance; §6.3.1 specifies single \
         inheritance for ObjectTypes and VariableTypes, and §7.10 that each ReferenceType shall \
         be the target of at most one HasSubtype reference.";
    HasChildCycle = "UA0206", Error,
        "following HasChild references from this node leads back to it",
        "OPC 10000-3 §7.5 states that starting from a node and only following references of the \
         subtypes of HasChild it shall never be possible to return to that node. HasComponent, \
         HasProperty and HasSubtype are all subtypes of HasChild. More than one path to the same \
         node is allowed; a cycle is not. Organizes is hierarchical but not a HasChild subtype, \
         so an Organizes loop is legal and not reported.";
    AbstractReferenceType = "UA0207", Error,
        "the reference is of an abstract ReferenceType",
        "OPC 10000-3 §5.3.1 defines IsAbstract on a ReferenceType as meaning that no reference of \
         that type shall exist, only of its subtypes; §7.2 repeats it for the References root. \
         Use a concrete subtype such as HasComponent or Organizes.";
    ReferenceNotAllowedForSourceClass = "UA0208", Error,
        "a node of this NodeClass may not be the source of this reference type",
        "Some NodeClasses restrict what they may reference at all. OPC 10000-3 §5.3.3.1 allows a \
         ReferenceType node to be the source of HasSubtype and HasProperty references only; \
         §5.8.3 allows a DataType node HasSubtype, HasProperty and HasEncoding only; §5.4 allows \
         a View to be the source of hierarchical references only.";
    HierarchicalSelfReference = "UA0209", Error,
        "a hierarchical reference points the node at itself",
        "OPC 10000-3 §7.3 states that it is not allowed for the source and the target of a \
         reference of a subtype of HierarchicalReferences to be the same node.";
    SymmetricWithInverseName = "UA0210", Error,
        "a symmetric ReferenceType states an InverseName",
        "OPC 10000-3 §5.3.2 states that if a ReferenceType is symmetric, the InverseName Attribute \
         shall be omitted: the meaning of the type is the same seen from either end, so there is no \
         inverse meaning to name, and both directions count as forward References. Either drop the \
         InverseName or clear Symmetric.";
    MissingInverseName = "UA0211", Error,
        "a non-symmetric ReferenceType states no InverseName",
        "OPC 10000-3 §5.3.2 states that if the ReferenceType is non-symmetric the InverseName \
         Attribute shall be set, naming the meaning of the type as seen from the TargetNode. \
         Table 9 lists the Attribute as optional because the condition is the Symmetric Attribute: \
         it is required exactly when Symmetric is false, and forbidden when it is true (UA0210).";
    TypeWithoutSupertype = "UA0212", Error,
        "the type node names no supertype",
        "OPC 10000-3 §5.3.3.3 states that a ReferenceType shall have exactly one supertype, \
         except for References, and §7.10 that each ReferenceType shall be the target of at most \
         one HasSubtype reference. OPC 10000-5 §6.2 requires every other ObjectType to inherit \
         from BaseObjectType directly or indirectly, and §7.2 and §12.2.1 give BaseVariableType \
         and BaseDataType the same role. Those four roots — i=31, i=58, i=62 and i=24 — aside, a \
         type that names no supertype sits outside its hierarchy. §5.3.3.3 adds that the subtype \
         is the end that has to state the reference.";
    ReferenceTargetTypeMismatch = "UA0213", Error,
        "the reference target is not of the type this reference type requires",
        "Some ReferenceTypes constrain the type of their target as well as its NodeClass. OPC \
         10000-3 §7.14 requires the target of a HasEncoding reference to be an Object of the \
         ObjectType DataTypeEncodingType (i=76) or one of its subtypes; §7.15 and §7.16 require \
         the target of GeneratesEvent and AlwaysGeneratesEvent to be an ObjectType representing \
         an EventType, that is BaseEventType (i=2041) or one of its subtypes.";

    MissingTypeDefinition = "UA0301", Error,
        "the Object or Variable has no HasTypeDefinition reference",
        "OPC 10000-3 §7.13 requires each Object and each Variable to be the source of exactly one \
         HasTypeDefinition reference; §5.5.1 and §5.6.2 restate it per NodeClass. An Object \
         defaults to BaseObjectType, a data variable to BaseDataVariableType, and a Property is \
         always PropertyType.";
    MultipleTypeDefinitions = "UA0302", Error,
        "the Object or Variable has more than one HasTypeDefinition reference",
        "OPC 10000-3 §7.13 requires exactly one HasTypeDefinition reference per Object and per \
         Variable. Keep the one that states the intended type and remove the rest.";
    AbstractTypeDefinition = "UA0303", Error,
        "the type definition is an abstract type",
        "OPC 10000-3 §5.5.2 and §5.6.5 define IsAbstract on an ObjectType or VariableType as \
         meaning that no instance of that type shall exist, only of its subtypes. Point the \
         instance at a concrete subtype. A node that carries a ModellingRule is exempt: §6.2.1 \
         permits the type of an instance declaration to be abstract, since only the materialised \
         instance has to be concrete, and §6.2.2 makes the ModellingRule the mark of a node that \
         exists to be instantiated rather than to be an instance — however the type reaches it.";
    TypeDefinitionClassMismatch = "UA0304", Error,
        "the type definition is of the wrong NodeClass for this node",
        "OPC 10000-3 §7.13: if the source of a HasTypeDefinition reference is an Object then the \
         target shall be an ObjectType; if the source is a Variable then the target shall be a \
         VariableType.";
    PropertyWithHierarchicalReference = "UA0305", Error,
        "a Property is the source of a hierarchical reference",
        "OPC 10000-3 §5.6.3 states that Properties are the leaf of any hierarchy and shall not be \
         the source of any hierarchical reference, so a Property neither contains Properties nor \
         exposes its structure; §7.3 repeats the prohibition for every subtype of \
         HierarchicalReferences. A Property may be the source of non-hierarchical references. If \
         the node needs children it is a data variable, referenced with HasComponent.";
    PropertyTypeDefinitionNotPropertyType = "UA0306", Error,
        "a Property's type definition is not PropertyType",
        "OPC 10000-3 §5.6.3 states that all Properties shall point to the PropertyType defined in \
         OPC 10000-5. PropertyType (i=68) may not be subtyped, so this is an equality, not a \
         subtype test. A Variable is a Property exactly when some node references it with \
         HasProperty.";
    ModellingRuleTargetInvalid = "UA0307", Error,
        "HasModellingRule does not name a ModellingRule",
        "OPC 10000-3 §7.12 requires the target of a HasModellingRule reference to be an Object of \
         the ObjectType ModellingRuleType (i=77) or one of its subtypes. The standard rules are \
         Mandatory, Optional, ExposesItsArray, OptionalPlaceholder and MandatoryPlaceholder.";
    ModellingRuleOnWrongNodeClass = "UA0308", Error,
        "only an Object, a Variable or a Method may carry a ModellingRule",
        "OPC 10000-3 §7.12 requires the source of a HasModellingRule reference to be an Object, a \
         Variable or a Method. A type node describes instances; it is not itself instantiated \
         under a modelling rule.";
    MissingMandatoryChild = "UA0309", Warning,
        "an instance does not answer for a Mandatory instance declaration of its type",
        "OPC 10000-3 §6.4.4.4.1 states that for a declaration marked Mandatory a similar node \
         shall exist on each instance, and §6.5 forbids adding a Mandatory child to a type while \
         instances lack it. §6.4.2 nonetheless lets a server hold part of an instance elsewhere, \
         and a nodeset file is a fragment, so this is reported as a warning. It is an error for a \
         Mandatory Property, because §4.5.2 requires a node and its Properties to reside \
         together.";
    MultipleModellingRules = "UA0310", Error,
        "the node has more than one HasModellingRule reference",
        "OPC 10000-3 §7.12 states that each node shall be the source of at most one \
         HasModellingRule reference.";
    PropertyTargetOfHasComponent = "UA0311", Error,
        "a Property is also referenced as a component",
        "OPC 10000-3 §5.6.3 distinguishes a Property from a data variable by the reference that \
         reaches it: a Property is the target of a HasProperty reference and shall not be the \
         target of any HasComponent reference. Choose one of the two for this Variable.";
    RedundantDeclarationOverride = "UA0312", Warning,
        "the type overrides an inherited instance declaration without changing anything",
        "OPC 10000-3 §6.3.3.3 states that a subtype should not override a Node unless it needs to \
         change it. This declaration repeats its supertype's type definition, ModellingRule and — \
         for a Variable — DataType, ValueRank and ArrayDimensions, so it adds nothing but a second \
         node to maintain. Neither end of a required override chain is reported: §6.3.3.3 allows a \
         nested declaration to be overridden only once the declaration above it has been, so an \
         override that anchors one beneath it is required, and an override beneath one hangs under \
         the new node rather than the inherited one and is therefore not a repetition at all.";
    MissingMandatoryPlaceholderChild = "UA0313", Warning,
        "an instance provides no child for a MandatoryPlaceholder declaration",
        "OPC 10000-3 §6.4.4.4.5 states that MandatoryPlaceholder requires each instance to provide \
         at least one instance with the TypeDefinition of the instance declaration or a subtype, \
         referenced with the same ReferenceType or a subtype; the BrowseName is not constrained, \
         so any such child answers for it. As with UA0309, §6.4.2 lets a server hold part of an \
         instance elsewhere and a nodeset file is a fragment, so this is a warning.";

    DataTypeNotADataType = "UA0401", Error,
        "the DataType attribute does not name a DataType node",
        "OPC 10000-3 §5.6.2 and §5.6.5 require the DataType attribute of a Variable or a \
         VariableType to be a valid NodeId of a DataType. An abstract DataType is allowed here: \
         §5.8.3 states that Variables and VariableTypes may point at one, only concrete values \
         may not be of it.";
    ArrayDimensionsRankMismatch = "UA0402", Error,
        "ArrayDimensions has a different number of entries than ValueRank",
        "OPC 10000-3 §5.6.5 requires the number of ArrayDimensions entries to equal the value of \
         ValueRank, and §5.6.2 requires it to equal the number of dimensions of the Value. Either \
         state one entry per dimension, or change the rank.";
    ArrayDimensionsWithoutRank = "UA0403", Warning,
        "ArrayDimensions is given although ValueRank does not fix a dimension count",
        "OPC 10000-3 §5.6.5 requires ArrayDimensions to be null when ValueRank is 0 or negative, \
         which is an error on a VariableType. §5.6.2 phrases the Variable rule against the Value \
         rather than the rank, and OPC 10000-5 §5.3 calls the behaviour server-specific, so on a \
         Variable this is reported as a warning.";
    UndefinedValueRank = "UA0404", Error,
        "ValueRank is below the smallest value the specification defines",
        "OPC 10000-3 §5.6.2 defines ValueRank as a fixed dimension count when it is 1 or greater, \
         and as OneOrMoreDimensions (0), Scalar (-1), Any (-2) or ScalarOrOneDimension (-3) \
         otherwise. Values below -3 have no meaning.";
    MissingArrayDimensions = "UA0405", Warning,
        "ValueRank fixes a dimension count but ArrayDimensions is absent",
        "OPC 10000-3 §5.6.5 requires a VariableType with a ValueRank above 0 to state one \
         ArrayDimensions entry per dimension, using 0 where the length is unknown. OPC 10000-5 \
         §5.3 states the same for Variables, which the specification itself frames less strictly, \
         so on a Variable this is reported as a warning.";
    NullDataType = "UA0406", Error,
        "the DataType attribute is null",
        "OPC 10000-3 §5.6.2 and §5.6.5 state that the DataType NodeId shall be a valid NodeId of \
         a DataType and shall not be null. Where no narrower type is known, BaseDataType (i=24) \
         is the correct value.";
    ArgumentPropertyNotArgumentArray = "UA0407", Error,
        "an InputArguments or OutputArguments Property of a Method does not hold an array of Argument",
        "OPC 10000-3 §5.7.1 Table 15 defines both Properties of a Method with the Data Type \
         Argument[], and the clause states that both contain an array of the DataType Argument as \
         specified in §8.6 — so the DataType is Argument (i=296) and the ValueRank is \
         OneDimension (1). OPC 10000-4 §5.12.2.2 has a client read that array to build the Call \
         request, argument by argument, which anything else makes impossible. Only a Variable a \
         Method references with HasProperty is checked: an ObjectType may hold a Property of the \
         same BrowseName for another purpose, as AuditUpdateMethodEventType does in OPC 10000-5.";
}

impl fmt::Display for DiagnosticCode {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for DiagnosticCode {
    type Err = ();

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::from_id(id).ok_or(())
    }
}
