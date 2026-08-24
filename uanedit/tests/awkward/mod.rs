//! Hand-written documents that put the lexical choices a nodeset is allowed to make in one place.
//!
//! They live here rather than beside the downloaded fixtures because those are gitignored, and a
//! round-trip test is worth nothing if its input is not in the repository.

/// Aliases, a Guid and an opaque NodeId, localized-text lists, an extension, an unknown element
/// and an unknown attribute, a value holding an ExtensionObject, CDATA, and comments in three
/// different places.
pub const AWKWARD: &str = r##"<?xml version="1.0" encoding="utf-8" ?>
<!-- a comment before the root element -->
<UANodeSet xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://opcfoundation.org/UA/2011/03/UANodeSet.xsd" LastModified='2026-02-03T04:05:06Z'>
  <NamespaceUris>
    <Uri>http://example.org/awkward/</Uri>
  </NamespaceUris>
  <Models>
    <Model ModelUri="http://example.org/awkward/" Version="1.0.0" PublicationDate="2026-02-03T00:00:00Z" ModelVersion="1.0.0">
      <RequiredModel ModelUri="http://opcfoundation.org/UA/" PublicationDate="2026-01-01T00:00:00Z" ModelVersion="1.5.4" />
    </Model>
  </Models>
  <Aliases>
    <Alias Alias="Organizes">i=35</Alias>
    <Alias Alias="HasTypeDefinition">i=40</Alias>
    <Alias Alias="String">i=12</Alias>
  </Aliases>
  <Extensions>
    <Extension>
      <vendor:Meta xmlns:vendor="http://example.org/vendor" Note="kept &amp; unchanged">
        <vendor:Payload><![CDATA[ <not xml> & raw ]]></vendor:Payload>
      </vendor:Meta>
    </Extension>
  </Extensions>
  <!-- a comment between the tables and the nodes -->
  <UAObject NodeId="ns=1;g=09087E75-8E5E-499B-954F-F2A9603DB28A" BrowseName="1:GuidNode" FutureAttribute="from a newer schema revision">
    <DisplayName>Guid node</DisplayName>
    <DisplayName Locale="de">Guid-Knoten</DisplayName>
    <Description Locale="">an empty locale is not an absent one</Description>
    <References>
      <Reference ReferenceType="Organizes" IsForward="false">i=85</Reference>
      <Reference ReferenceType="HasTypeDefinition">i=58</Reference>
    </References>
    <vendor:Annotation xmlns:vendor="http://example.org/vendor">an element this schema does not define</vendor:Annotation>
  </UAObject>
  <!-- a comment between two nodes -->
  <UAVariable NodeId="ns=1;b=M/RbKBsRVkePCePcx24oRA==" BrowseName="1:OpaqueNode" ParentNodeId="ns=1;g=09087E75-8E5E-499B-954F-F2A9603DB28A" DataType="String" ValueRank="1" ArrayDimensions="0">
    <DisplayName>Opaque node</DisplayName>
    <References>
      <Reference ReferenceType="HasTypeDefinition">i=68</Reference>
    </References>
    <Value>
      <ListOfExtensionObject xmlns="http://opcfoundation.org/UA/2008/02/Types.xsd">
        <ExtensionObject>
          <TypeId>
            <Identifier>i=297</Identifier>
          </TypeId>
          <Body>
            <Argument>
              <Name>Input</Name>
              <DataType>
                <Identifier>i=12</Identifier>
              </DataType>
              <ValueRank>-1</ValueRank>
              <ArrayDimensions />
              <Description>
                <Locale />
                <Text>an &lt;argument&gt; &amp; a description</Text>
              </Description>
            </Argument>
          </Body>
        </ExtensionObject>
      </ListOfExtensionObject>
    </Value>
  </UAVariable>
  <UADataType NodeId="ns=1;s=Colour" BrowseName="1:Colour">
    <DisplayName>Colour</DisplayName>
    <References>
      <Reference ReferenceType="HasSubtype" IsForward="false">i=29</Reference>
    </References>
    <Definition Name="1:Colour">
      <Field Name="Red" Value="1">
        <Description>a value &gt; zero</Description>
      </Field>
      <Field Name="Green" Value="2" />
    </Definition>
  </UADataType>
  <UAObject NodeId="ns=1;i=7" BrowseName="1:SelfClosing" />
</UANodeSet>
<!-- a comment after the root element -->
"##;

/// A byte-order mark, CRLF line endings, tab indentation, a processing instruction, and a
/// declaration that names its encoding and standalone flag.
pub const BOM_CRLF: &str = concat!(
    "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
    "<?some-tool generated=\"yes\"?>\r\n",
    "<UANodeSet xmlns=\"http://opcfoundation.org/UA/2011/03/UANodeSet.xsd\">\r\n",
    "\t<NamespaceUris>\r\n",
    "\t\t<Uri>http://example.org/tabs/</Uri>\r\n",
    "\t</NamespaceUris>\r\n",
    "\t<Aliases>\r\n",
    "\t\t<Alias Alias=\"HasTypeDefinition\">i=40</Alias>\r\n",
    "\t</Aliases>\r\n",
    "\t<UAObject NodeId=\"ns=1;i=1\" BrowseName=\"1:Tabbed\">\r\n",
    "\t\t<DisplayName>Tabbed</DisplayName>\r\n",
    "\t\t<References>\r\n",
    "\t\t\t<Reference ReferenceType=\"HasTypeDefinition\">i=58</Reference>\r\n",
    "\t\t</References>\r\n",
    "\t</UAObject>\r\n",
    "</UANodeSet>\r\n",
);

/// The lexical choices a save has to replay rather than re-spell: an attribute order this crate
/// does not use, an `xs:double` and an `xs:unsignedInt` written the long way, padding the schema's
/// token types allow, child elements written empty rather than left out, an element the schema does
/// not define sitting between two that it does, and comments in front of nodes.
pub const LEXICAL: &str = r##"<?xml version="1.0" encoding="utf-8" ?>
<UANodeSet xmlns="http://opcfoundation.org/UA/2011/03/UANodeSet.xsd">
  <NamespaceUris>
    <Uri>http://example.org/lexical/</Uri>
  </NamespaceUris>
  <!-- a comment in front of the first node -->
  <UAVariable BrowseName="1:Odd" DataType="i=6" NodeId="ns=1;i=1" AccessLevel="03" MinimumSamplingInterval="1.0E3" ValueRank="2" ArrayDimensions=" 2,3 ">
    <DisplayName>Odd</DisplayName>
    <vendor:Note xmlns:vendor="http://example.org/vendor">between the name and the references</vendor:Note>
    <References />
    <RolePermissions />
    <Extensions />
  </UAVariable>
  <!-- a comment in front of the node that gets deleted -->
  <UAObject NodeId="ns=1;i=2" BrowseName="1:Doomed" />
  <UAObject NodeId="ns=1;i=3" BrowseName="1:Survivor" />
</UANodeSet>
"##;

/// The schema's second root element, which describes edits to a nodeset rather than a nodeset.
pub const CHANGES: &str = r##"<?xml version="1.0" encoding="utf-8" ?>
<UANodeSetChanges xmlns="http://opcfoundation.org/UA/2011/03/UANodeSet.xsd" TransactionId="1">
  <NodesToDelete>
    <Node>ns=1;i=1</Node>
  </NodesToDelete>
</UANodeSetChanges>
"##;
